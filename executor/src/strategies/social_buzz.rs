use crate::strategies::{Strategy, MarketEvent, StrategyAction, OrderDetails, EventType};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashSet, HashMap, VecDeque};
use tracing::info;
use shared_models::{Side, RiskMetrics};
use chrono::{DateTime, Utc, Duration};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SocialBuzz {
    lookback_minutes: u32,
    std_dev_threshold: f64,
    min_engagement_score: f64,
    #[serde(skip)]
    mention_history: HashMap<String, VecDeque<MentionData>>,
    #[serde(skip)]
    last_trade: HashMap<String, DateTime<Utc>>,
}

#[derive(Debug, Clone)]
struct MentionData {
    timestamp: DateTime<Utc>,
    mentions: u32,
    sentiment: f64,
    engagement: f64,
    influencer_score: f64,
}

#[async_trait]
impl Strategy for SocialBuzz {
    fn id(&self) -> &'static str { "social_buzz" }
    
    fn subscriptions(&self) -> HashSet<EventType> {
        [EventType::Social, EventType::TwitterRaw, EventType::FarcasterRaw].iter().cloned().collect()
    }

    async fn init(&mut self, params: &Value) -> Result<()> {
        #[derive(Deserialize)]
        struct Params {
            lookback_minutes: u32,
            std_dev_threshold: f64,
            min_engagement_score: f64,
        }
        
        let p: Params = serde_json::from_value(params.clone())?;
        self.lookback_minutes = p.lookback_minutes;
        self.std_dev_threshold = p.std_dev_threshold;
        self.min_engagement_score = p.min_engagement_score;
        
        info!(
            strategy = self.id(),
            lookback = self.lookback_minutes,
            threshold = self.std_dev_threshold,
            "Social buzz strategy initialized"
        );
        
        Ok(())
    }

    async fn on_event(&mut self, event: &MarketEvent) -> Result<StrategyAction> {
        match event {
            MarketEvent::Social(mention) => {
                let history = self.mention_history
                    .entry(mention.token_address.clone())
                    .or_insert_with(VecDeque::new);
                
                // Aggregate data
                let data = MentionData {
                    timestamp: mention.timestamp,
                    mentions: mention.mentions_1h,
                    sentiment: mention.sentiment,
                    engagement: mention.engagement_score,
                    influencer_score: mention.influencer_score,
                };
                
                history.push_back(data);
                
                // Clean old data
                let cutoff = mention.timestamp - Duration::minutes(self.lookback_minutes as i64);
                while let Some(old) = history.front() {
                    if old.timestamp < cutoff {
                        history.pop_front();
                    } else {
                        break;
                    }
                }
                
                // Need sufficient history
                if history.len() < 5 {
                    return Ok(StrategyAction::Hold);
                }
                
                // Check cooldown
                if let Some(last) = self.last_trade.get(&mention.token_address) {
                    if mention.timestamp.signed_duration_since(*last) < Duration::hours(1) {
                        return Ok(StrategyAction::Hold);
                    }
                }
                
                // Calculate buzz metrics
                let recent_mentions: Vec<u32> = history.iter()
                    .rev()
                    .take(3)
                    .map(|d| d.mentions)
                    .collect();
                
                let historical_mentions: Vec<u32> = history.iter()
                    .skip(3)
                    .map(|d| d.mentions)
                    .collect();
                
                if historical_mentions.is_empty() {
                    return Ok(StrategyAction::Hold);
                }
                
                let recent_avg = recent_mentions.iter().sum::<u32>() as f64 / recent_mentions.len() as f64;
                let historical_avg = historical_mentions.iter().sum::<u32>() as f64 / historical_mentions.len() as f64;
                
                if historical_avg == 0.0 {
                    return Ok(StrategyAction::Hold);
                }
                
                let buzz_ratio = recent_avg / historical_avg;
                let latest = history.back().unwrap();
                
                // Calculate weighted score
                let buzz_score = buzz_ratio * latest.sentiment * latest.engagement;
                
                // Signal when significant buzz increase detected
                if buzz_ratio > self.std_dev_threshold && 
                   latest.engagement > self.min_engagement_score &&
                   latest.sentiment > 0.5 {
                    
                    info!(
                        strategy = self.id(),
                        token = %mention.token_address,
                        buzz_ratio = format!("{:.2}x", buzz_ratio),
                        sentiment = latest.sentiment,
                        engagement = latest.engagement,
                        "SOCIAL BUZZ BUY signal detected"
                    );
                    
                    self.last_trade.insert(mention.token_address.clone(), mention.timestamp);
                    
                    let confidence = ((buzz_score / 10.0) * 0.8).min(0.9);
                    
                    let order = OrderDetails {
                        token_address: mention.token_address.clone(),
                        symbol: format!("MEME_{}", &mention.token_address[..6]),
                        suggested_size_usd: 45.0,
                        confidence,
                        side: Side::Long,
                        strategy_metadata: HashMap::from([
                            ("buzz_ratio".to_string(), serde_json::json!(buzz_ratio)),
                            ("sentiment".to_string(), serde_json::json!(latest.sentiment)),
                            ("engagement".to_string(), serde_json::json!(latest.engagement)),
                            ("influencer_score".to_string(), serde_json::json!(latest.influencer_score)),
                        ]),
                        risk_metrics: RiskMetrics {
                            position_size_pct: 0.018,
                            stop_loss_price: None, // Will use trailing stop
                            take_profit_price: None, // Dynamic exit
                            max_slippage_bps: 40,
                            time_limit_seconds: Some(180),
                        },
                    };
                    
                    return Ok(StrategyAction::Execute(order));
                }
            }
            MarketEvent::TwitterRaw(tweet) => {
                // Process raw tweets for token mentions
                // Extract token addresses from tweet text
                // This would use regex patterns for Solana addresses
            }
            MarketEvent::FarcasterRaw(cast) => {
                // Process Farcaster casts similarly
            }
            _ => {}
        }
        
        Ok(StrategyAction::Hold)
    }
    
    fn get_state(&self) -> Value {
        serde_json::json!({
            "lookback_minutes": self.lookback_minutes,
            "std_dev_threshold": self.std_dev_threshold,
            "tracked_tokens": self.mention_history.len(),
            "recent_trades": self.last_trade.len(),
        })
    }
}
            MarketEvent::FarcasterRaw(cast) => {
                // Similar processing for Farcaster casts
                let solana_address_pattern = regex::Regex::new(r"[1-9A-HJ-NP-Za-km-z]{32,44}").unwrap();
                
                if let Some(addr_match) = solana_address_pattern.find(&cast.text) {
                    let token_address = addr_match.as_str();
                    
                    let synthetic_mention = shared_models::SocialMention {
                        token_address: token_address.to_string(),
                        source: "farcaster".to_string(),
                        sentiment: if cast.text.contains("bullish") || cast.text.contains("gem") { 0.7 } else { 0.5 },
                        engagement_score: (cast.author_followers as f64 / 1000.0).min(1.0),
                        influencer_score: (cast.author_followers as f64 / 5000.0).min(1.0),
                        mentions_1h: 1,
                        timestamp: shared_models::DateTime::from_timestamp(cast.timestamp, 0).unwrap_or_else(chrono::Utc::now),
                    };
                    
                    return self.on_event(&MarketEvent::Social(synthetic_mention)).await;
                }
            }
            _ => {}
        }
        
        Ok(StrategyAction::Hold)
    }
    
    fn get_state(&self) -> Value {
        serde_json::json!({
            "lookback_minutes": self.lookback_minutes,
            "std_dev_threshold": self.std_dev_threshold,
            "tracked_tokens": self.mention_history.len(),
            "recent_trades": self.last_trade.len(),
        })
    }
}
