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
pub struct Momentum5m {
    lookback: usize,
    vol_multiplier: f64,
    price_change_threshold: f64,
    #[serde(skip)]
    price_history: HashMap<String, VecDeque<(DateTime<Utc>, f64, f64)>>, // (time, price, volume)
    #[serde(skip)]
    last_signal: HashMap<String, DateTime<Utc>>,
}

#[async_trait]
impl Strategy for Momentum5m {
    fn id(&self) -> &'static str { "momentum_5m" }
    
    fn subscriptions(&self) -> HashSet<EventType> {
        [EventType::Price, EventType::Volume].iter().cloned().collect()
    }

    async fn init(&mut self, params: &Value) -> Result<()> {
        #[derive(Deserialize)]
        struct Params {
            lookback: usize,
            vol_multiplier: f64,
            price_change_threshold: f64,
        }
        
        let p: Params = serde_json::from_value(params.clone())?;
        self.lookback = p.lookback;
        self.vol_multiplier = p.vol_multiplier;
        self.price_change_threshold = p.price_change_threshold;
        
        info!(
            strategy = self.id(),
            lookback = self.lookback,
            vol_multiplier = self.vol_multiplier,
            threshold = self.price_change_threshold,
            "Momentum strategy initialized"
        );
        
        Ok(())
    }

    async fn on_event(&mut self, event: &MarketEvent) -> Result<StrategyAction> {
        match event {
            MarketEvent::Price(tick) => {
                let history = self.price_history
                    .entry(tick.token_address.clone())
                    .or_insert_with(|| VecDeque::with_capacity(self.lookback));
                
                // Add new data point
                history.push_back((tick.timestamp, tick.price_usd, tick.volume_usd_5m));
                
                // Keep only lookback period
                while history.len() > self.lookback {
                    history.pop_front();
                }
                
                // Need full history for signal
                if history.len() < self.lookback {
                    return Ok(StrategyAction::Hold);
                }
                
                // Check for cooldown
                if let Some(last_time) = self.last_signal.get(&tick.token_address) {
                    if tick.timestamp.signed_duration_since(*last_time) < Duration::minutes(15) {
                        return Ok(StrategyAction::Hold);
                    }
                }
                
                // Calculate momentum
                let oldest = history.front().unwrap();
                let newest = history.back().unwrap();
                
                let price_change = (newest.1 - oldest.1) / oldest.1;
                let avg_volume: f64 = history.iter().map(|(_, _, v)| v).sum::<f64>() / history.len() as f64;
                let current_vol_ratio = newest.2 / avg_volume;
                
                // Momentum signal: price increase + volume surge
                if price_change > self.price_change_threshold && 
                   current_vol_ratio > self.vol_multiplier &&
                   tick.liquidity_usd > 50000.0 { // Minimum liquidity check
                    
                    info!(
                        strategy = self.id(),
                        token = %tick.token_address,
                        price_change = format!("{:.2}%", price_change * 100.0),
                        volume_ratio = format!("{:.2}x", current_vol_ratio),
                        "MOMENTUM BUY signal detected"
                    );
                    
                    self.last_signal.insert(tick.token_address.clone(), tick.timestamp);
                    
                    // Dynamic confidence based on signal strength
                    let confidence = (0.5 + (price_change / self.price_change_threshold * 0.25) +
                                     (current_vol_ratio / self.vol_multiplier * 0.25))
                                     .min(0.95);
                    
                    let order = OrderDetails {
                        token_address: tick.token_address.clone(),
                        symbol: format!("MEME_{}", &tick.token_address[..6]),
                        suggested_size_usd: 50.0, // Base size, will be adjusted by risk manager
                        confidence,
                        side: Side::Long,
                        strategy_metadata: HashMap::from([
                            ("price_change".to_string(), serde_json::json!(price_change)),
                            ("volume_ratio".to_string(), serde_json::json!(current_vol_ratio)),
                            ("liquidity".to_string(), serde_json::json!(tick.liquidity_usd)),
                        ]),
                        risk_metrics: RiskMetrics {
                            position_size_pct: 0.02, // 2% of portfolio
                            stop_loss_price: Some(tick.price_usd * 0.95), // 5% stop loss
                            take_profit_price: Some(tick.price_usd * 1.10), // 10% take profit
                            max_slippage_bps: 50,
                            time_limit_seconds: Some(300), // 5 minute execution window
                        },
                    };
                    
                    return Ok(StrategyAction::Execute(order));
                }
            }
            _ => {}
        }
        
        Ok(StrategyAction::Hold)
    }
    
    fn get_state(&self) -> Value {
        serde_json::json!({
            "lookback": self.lookback,
            "vol_multiplier": self.vol_multiplier,
            "price_change_threshold": self.price_change_threshold,
            "tracked_tokens": self.price_history.len(),
            "active_signals": self.last_signal.len(),
        })
    }
}
