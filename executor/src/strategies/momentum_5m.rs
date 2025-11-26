use crate::strategies::{EventType, MarketEvent, OrderDetails, Strategy, StrategyAction};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared_models::{RiskMetrics, Side};
use std::collections::{HashMap, HashSet, VecDeque};
use tracing::info;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Momentum5m {
    lookback: usize,
    vol_multiplier: f64,
    price_change_threshold: f64,
    regime_sma_lookback: usize,
    cooldown_minutes: i64,
    min_liquidity_usd: f64,
    allowed_tokens: Option<HashSet<String>>,
    #[serde(skip)]
    price_history: HashMap<String, VecDeque<(DateTime<Utc>, f64, f64)>>, // (time, price, volume)
    #[serde(skip)]
    last_signal: HashMap<String, DateTime<Utc>>,
}

#[async_trait]
impl Strategy for Momentum5m {
    fn id(&self) -> &'static str {
        "momentum_5m"
    }

    fn subscriptions(&self) -> HashSet<EventType> {
        [EventType::Price, EventType::Volume]
            .iter()
            .cloned()
            .collect()
    }

    async fn init(&mut self, params: &Value) -> Result<()> {
        #[derive(Deserialize)]
        struct Params {
            lookback: usize,
            vol_multiplier: f64,
            price_change_threshold: f64,
            #[serde(default = "default_regime_sma")]
            regime_sma_lookback: usize,
            #[serde(default = "default_cooldown")]
            cooldown_minutes: i64,
            #[serde(default = "default_min_liq")]
            min_liquidity_usd: f64,
            #[serde(default)]
            allowed_tokens: Option<Vec<String>>,
        }

        let p: Params = serde_json::from_value(params.clone())?;
        self.lookback = p.lookback;
        self.vol_multiplier = p.vol_multiplier;
        self.price_change_threshold = p.price_change_threshold;
        self.regime_sma_lookback = p.regime_sma_lookback;
        self.cooldown_minutes = p.cooldown_minutes;
        self.min_liquidity_usd = p.min_liquidity_usd;
        self.allowed_tokens = p.allowed_tokens.map(|tokens| tokens.into_iter().collect());

        let allow_count = self
            .allowed_tokens
            .as_ref()
            .map(|set| set.len())
            .unwrap_or(0);

        info!(
            strategy = self.id(),
            lookback = self.lookback,
            vol_multiplier = self.vol_multiplier,
            threshold = self.price_change_threshold,
            regime_sma = self.regime_sma_lookback,
            cooldown = self.cooldown_minutes,
            min_liq = self.min_liquidity_usd,
            allow_count,
            "Momentum strategy initialized"
        );

        Ok(())
    }

    async fn on_event(&mut self, event: &MarketEvent) -> Result<StrategyAction> {
        if let MarketEvent::Price(tick) = event {
            if let Some(allowed) = &self.allowed_tokens {
                if !allowed.contains(&tick.token_address) {
                    return Ok(StrategyAction::Hold);
                }
            }

            let history = self
                .price_history
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
                if tick.timestamp.signed_duration_since(*last_time)
                    < Duration::minutes(self.cooldown_minutes)
                {
                    return Ok(StrategyAction::Hold);
                }
            }

            let oldest = match history.front() {
                Some(data) => data,
                None => return Ok(StrategyAction::Hold), // Not enough data
            };
            let newest = match history.back() {
                Some(data) => data,
                None => return Ok(StrategyAction::Hold), // Not enough data
            };

            let price_change = (newest.1 - oldest.1) / oldest.1;
            let avg_volume: f64 =
                history.iter().map(|(_, _, v)| v).sum::<f64>() / history.len() as f64;
            let current_vol_ratio = newest.2 / avg_volume;

            // Momentum signal: price increase + volume surge + regime gate
            if price_change > self.price_change_threshold
                && current_vol_ratio > self.vol_multiplier
                && tick.liquidity_usd > self.min_liquidity_usd
                && regime_ok(history, self.regime_sma_lookback)
            {
                // Minimum liquidity check

                info!(
                    strategy = self.id(),
                    token = %tick.token_address,
                    price_change = format!("{:.2}%", price_change * 100.0),
                    volume_ratio = format!("{:.2}x", current_vol_ratio),
                    "MOMENTUM BUY signal detected"
                );

                self.last_signal
                    .insert(tick.token_address.clone(), tick.timestamp);

                // Dynamic confidence based on signal strength
                let confidence = (0.5
                    + (price_change / self.price_change_threshold * 0.25)
                    + (current_vol_ratio / self.vol_multiplier * 0.25))
                    .min(0.95);

                let symbol_suffix: String = tick.token_address.chars().take(6).collect();
                let order = OrderDetails {
                    token_address: tick.token_address.clone(),
                    symbol: format!("MEME_{}", symbol_suffix),
                    suggested_size_usd: 40.0, // Tighter base; RiskManager can scale
                    confidence,
                    side: Side::Long,
                    strategy_metadata: HashMap::from([
                        ("price_change".to_string(), serde_json::json!(price_change)),
                        (
                            "volume_ratio".to_string(),
                            serde_json::json!(current_vol_ratio),
                        ),
                        (
                            "liquidity".to_string(),
                            serde_json::json!(tick.liquidity_usd),
                        ),
                    ]),
                    risk_metrics: RiskMetrics {
                        position_size_pct: 0.02,                        // 2% of portfolio
                        stop_loss_price: Some(tick.price_usd * 0.97),   // 3% stop loss
                        take_profit_price: Some(tick.price_usd * 1.06), // 6% take profit
                        max_slippage_bps: 50,
                        time_limit_seconds: Some(300), // 5 minute execution window
                    },
                };

                return Ok(StrategyAction::Execute(order));
            }
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
            "allowed_tokens": self.allowed_tokens.as_ref().map(|set| {
                set.iter().cloned().collect::<Vec<String>>()
            }),
        })
    }
}

fn regime_ok(history: &VecDeque<(DateTime<Utc>, f64, f64)>, lookback: usize) -> bool {
    if lookback == 0 || history.len() < lookback {
        return true;
    }
    let closes: Vec<f64> = history.iter().map(|(_, p, _)| *p).collect();
    let recent = closes.last().copied().unwrap_or(0.0);
    let sma: f64 = closes[closes.len() - lookback..].iter().sum::<f64>() / lookback as f64;
    recent >= sma
}

fn default_regime_sma() -> usize {
    24
} // default 24 bars ≈ 2h for 5m bars
fn default_cooldown() -> i64 {
    15
}
fn default_min_liq() -> f64 {
    50_000.0
}
