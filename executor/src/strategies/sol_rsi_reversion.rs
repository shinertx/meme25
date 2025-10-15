use crate::strategies::{EventType, MarketEvent, OrderDetails, Strategy, StrategyAction};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared_models::{RiskMetrics, Side};
use std::collections::{HashMap, HashSet, VecDeque};
use tracing::info;

const MAX_HISTORY_LEN: usize = 256;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SolRsiReversion {
    rsi_length: usize,
    oversold_level: f64,
    exit_level: f64,
    min_drop_pct: f64,
    drop_lookback_minutes: i64,
    min_recent_drop_pct: f64,
    bounce_take_profit_pct: f64,
    hard_stop_pct: f64,
    max_hold_minutes: i64,
    cooldown_minutes: i64,
    trade_size_usd: f64,
    min_liquidity_usd: f64,
    min_volume_usd_5m: f64,
    #[serde(default)]
    allowed_tokens: Option<Vec<String>>,
    #[serde(skip)]
    allowed_token_set: HashSet<String>,
    #[serde(skip)]
    price_history: HashMap<String, VecDeque<(DateTime<Utc>, f64)>>,
    #[serde(skip)]
    open_positions: HashMap<String, ActiveTrade>,
    #[serde(skip)]
    last_exit: HashMap<String, DateTime<Utc>>,
}

#[derive(Debug, Clone)]
struct ActiveTrade {
    entry_price: f64,
    entry_time: DateTime<Utc>,
    entry_rsi: f64,
    entry_drop_pct: f64,
}

#[async_trait]
impl Strategy for SolRsiReversion {
    fn id(&self) -> &'static str {
        "sol_rsi_reversion"
    }

    fn subscriptions(&self) -> HashSet<EventType> {
        [EventType::Price].into_iter().collect()
    }

    async fn init(&mut self, params: &Value) -> Result<()> {
        #[derive(Deserialize)]
        struct Params {
            #[serde(default = "default_rsi_length")]
            rsi_length: usize,
            #[serde(default = "default_oversold")]
            oversold_level: f64,
            #[serde(default = "default_exit")]
            exit_level: f64,
            #[serde(default = "default_min_drop_pct")]
            min_drop_pct: f64,
            #[serde(default = "default_drop_lookback")]
            drop_lookback_minutes: i64,
            #[serde(default = "default_recent_drop_pct")]
            min_recent_drop_pct: f64,
            #[serde(default = "default_bounce_take_profit")]
            bounce_take_profit_pct: f64,
            #[serde(default = "default_hard_stop_pct")]
            hard_stop_pct: f64,
            #[serde(default = "default_max_hold_minutes")]
            max_hold_minutes: i64,
            #[serde(default = "default_cooldown_minutes")]
            cooldown_minutes: i64,
            #[serde(default = "default_trade_size")]
            trade_size_usd: f64,
            #[serde(default = "default_min_liquidity")]
            min_liquidity_usd: f64,
            #[serde(default = "default_min_volume")]
            min_volume_usd_5m: f64,
            #[serde(default)]
            allowed_tokens: Option<Vec<String>>,
        }

        let parsed: Params = serde_json::from_value(params.clone())?;
        self.rsi_length = parsed.rsi_length.max(3);
        self.oversold_level = parsed.oversold_level;
        self.exit_level = parsed.exit_level;
        self.min_drop_pct = parsed.min_drop_pct.max(0.005);
        self.drop_lookback_minutes = parsed.drop_lookback_minutes.max(15);
        self.min_recent_drop_pct = parsed.min_recent_drop_pct.max(0.5);
        self.bounce_take_profit_pct = parsed.bounce_take_profit_pct.max(0.005);
        self.hard_stop_pct = parsed.hard_stop_pct.max(0.01);
        self.max_hold_minutes = parsed.max_hold_minutes.max(30);
        self.cooldown_minutes = parsed.cooldown_minutes.max(30);
        self.trade_size_usd = parsed.trade_size_usd.max(10.0);
        self.min_liquidity_usd = parsed.min_liquidity_usd.max(1_000_000.0);
        self.min_volume_usd_5m = parsed.min_volume_usd_5m.max(250_000.0);
        self.allowed_token_set = parsed
            .allowed_tokens
            .map(|tokens| tokens.into_iter().collect())
            .unwrap_or_else(default_allowed_token_set);

        info!(
            strategy = self.id(),
            rsi_length = self.rsi_length,
            oversold = self.oversold_level,
            exit_level = self.exit_level,
            min_drop_pct = self.min_drop_pct,
            drop_lookback = self.drop_lookback_minutes,
            min_recent_drop_pct = self.min_recent_drop_pct,
            hard_stop_pct = self.hard_stop_pct,
            max_hold_minutes = self.max_hold_minutes,
            cooldown_minutes = self.cooldown_minutes,
            min_liquidity = self.min_liquidity_usd,
            min_volume_5m = self.min_volume_usd_5m,
            allowed_tokens = self.allowed_token_set.len(),
            "Sol RSI reversion strategy initialized"
        );

        Ok(())
    }

    async fn on_event(&mut self, event: &MarketEvent) -> Result<StrategyAction> {
        let tick = match event {
            MarketEvent::Price(tick) => tick,
            _ => return Ok(StrategyAction::Hold),
        };

        if !self.allowed_token_set.is_empty() && !self.allowed_token_set.contains(&tick.token_address)
        {
            return Ok(StrategyAction::Hold);
        }

        if tick.liquidity_usd < self.min_liquidity_usd || tick.volume_usd_5m < self.min_volume_usd_5m
        {
            return Ok(StrategyAction::Hold);
        }

        let history = self
            .price_history
            .entry(tick.token_address.clone())
            .or_insert_with(VecDeque::new);
        history.push_back((tick.timestamp, tick.price_usd));
        while history.len() > MAX_HISTORY_LEN {
            history.pop_front();
        }

        let rsi_opt = calculate_rsi(history, self.rsi_length);

        // manage open positions
        if let Some(position) = self.open_positions.get(&tick.token_address) {
            if let Some(rsi_value) = rsi_opt {
                let holding_minutes = tick
                    .timestamp
                    .signed_duration_since(position.entry_time)
                    .num_minutes();
                let bounce = (tick.price_usd / position.entry_price) - 1.0;
                let should_exit = rsi_value >= self.exit_level
                    || holding_minutes >= self.max_hold_minutes
                    || bounce >= self.bounce_take_profit_pct;

                if should_exit {
                    info!(
                        strategy = self.id(),
                        token = %tick.token_address,
                        current_rsi = rsi_value,
                        holding_minutes,
                        bounce = format!("{:.2}%", bounce * 100.0),
                        "RSI mean reversion exit signal"
                    );
                    self.open_positions.remove(&tick.token_address);
                    self.last_exit
                        .insert(tick.token_address.clone(), tick.timestamp);
                    return Ok(StrategyAction::ClosePosition);
                }
            }

            return Ok(StrategyAction::Hold);
        }

        // check cooldown before new entries
        if let Some(last_exit) = self.last_exit.get(&tick.token_address) {
            if tick
                .timestamp
                .signed_duration_since(*last_exit)
                .num_minutes()
                < self.cooldown_minutes
            {
                return Ok(StrategyAction::Hold);
            }
        }

        let rsi_value = match rsi_opt {
            Some(value) => value,
            None => return Ok(StrategyAction::Hold),
        };

        if rsi_value >= self.oversold_level {
            return Ok(StrategyAction::Hold);
        }

        if tick.price_change_5m > -(self.min_recent_drop_pct) {
            return Ok(StrategyAction::Hold);
        }

        let drop_pct_opt = percentage_change_since(
            history,
            tick.timestamp,
            Duration::minutes(self.drop_lookback_minutes),
        );
        let drop_pct = match drop_pct_opt {
            Some(pct) if pct <= -self.min_drop_pct => pct,
            _ => return Ok(StrategyAction::Hold),
        };

        let confidence = {
            let rsi_gap = (self.oversold_level - rsi_value).max(0.0);
            let drop_severity = (-drop_pct).max(0.0);
            (0.45 + (rsi_gap / 50.0) + (drop_severity / self.min_drop_pct * 0.15)).min(0.9)
        };

        info!(
            strategy = self.id(),
            token = %tick.token_address,
            rsi = rsi_value,
            drop_pct = format!("{:.2}%", drop_pct * 100.0),
            price_change_5m = tick.price_change_5m,
            liquidity = tick.liquidity_usd,
            volume_5m = tick.volume_usd_5m,
            "RSI mean reversion entry signal"
        );

        self.open_positions.insert(
            tick.token_address.clone(),
            ActiveTrade {
                entry_price: tick.price_usd,
                entry_time: tick.timestamp,
                entry_rsi: rsi_value,
                entry_drop_pct: drop_pct,
            },
        );

        let symbol_suffix: String = tick.token_address.chars().take(6).collect();
        let order = OrderDetails {
            token_address: tick.token_address.clone(),
            symbol: format!("SOL_{}", symbol_suffix),
            suggested_size_usd: self.trade_size_usd,
            confidence,
            side: Side::Long,
            strategy_metadata: HashMap::from([
                ("entry_rsi".to_string(), serde_json::json!(rsi_value)),
                (
                    "drop_pct".to_string(),
                    serde_json::json!(drop_pct),
                ),
                (
                    "lookback_minutes".to_string(),
                    serde_json::json!(self.drop_lookback_minutes),
                ),
            ]),
            risk_metrics: RiskMetrics {
                position_size_pct: 0.015,
                stop_loss_price: Some(tick.price_usd * (1.0 - self.hard_stop_pct)),
                take_profit_price: Some(tick.price_usd * (1.0 + self.bounce_take_profit_pct)),
                max_slippage_bps: 40,
                time_limit_seconds: Some(180),
            },
        };

        Ok(StrategyAction::Execute(order))
    }

    fn get_state(&self) -> Value {
        serde_json::json!({
            "rsi_length": self.rsi_length,
            "oversold_level": self.oversold_level,
            "exit_level": self.exit_level,
            "min_drop_pct": self.min_drop_pct,
            "drop_lookback_minutes": self.drop_lookback_minutes,
            "min_recent_drop_pct": self.min_recent_drop_pct,
            "open_positions": self.open_positions.len(),
            "tracked_tokens": self.price_history.len(),
        })
    }
}

fn calculate_rsi(history: &VecDeque<(DateTime<Utc>, f64)>, period: usize) -> Option<f64> {
    if history.len() < period + 1 {
        return None;
    }

    let prices: Vec<f64> = history.iter().map(|(_, price)| *price).collect();
    let tail = &prices[prices.len() - (period + 1)..];
    let mut gain_sum = 0.0;
    let mut loss_sum = 0.0;
    for window in tail.windows(2) {
        let change = window[1] - window[0];
        if change > 0.0 {
            gain_sum += change;
        } else {
            loss_sum += -change;
        }
    }

    if gain_sum == 0.0 && loss_sum == 0.0 {
        return Some(50.0);
    }

    if loss_sum == 0.0 {
        return Some(100.0);
    }

    let avg_gain = gain_sum / period as f64;
    let avg_loss = loss_sum / period as f64;
    let rs = if avg_loss.abs() < f64::EPSILON {
        return Some(100.0);
    } else {
        avg_gain / avg_loss
    };
    Some(100.0 - (100.0 / (1.0 + rs)))
}

fn percentage_change_since(
    history: &VecDeque<(DateTime<Utc>, f64)>,
    current_time: DateTime<Utc>,
    lookback: Duration,
) -> Option<f64> {
    let target_time = current_time - lookback;
    let mut past_price = None;
    for (timestamp, price) in history.iter().rev() {
        if *timestamp <= target_time {
            past_price = Some(*price);
            break;
        }
    }
    let past = past_price?;
    let current = history.back()?.1;
    if past.abs() < f64::EPSILON {
        return None;
    }
    Some((current / past) - 1.0)
}

fn default_rsi_length() -> usize {
    14
}

fn default_oversold() -> f64 {
    18.0
}

fn default_exit() -> f64 {
    45.0
}

fn default_min_drop_pct() -> f64 {
    0.012
}

fn default_drop_lookback() -> i64 {
    60
}

fn default_recent_drop_pct() -> f64 {
    1.0
}

fn default_bounce_take_profit() -> f64 {
    0.03
}

fn default_hard_stop_pct() -> f64 {
    0.035
}

fn default_max_hold_minutes() -> i64 {
    180
}

fn default_cooldown_minutes() -> i64 {
    180
}

fn default_trade_size() -> f64 {
    45.0
}

fn default_min_liquidity() -> f64 {
    5_000_000.0
}

fn default_min_volume() -> f64 {
    2_000_000.0
}

fn default_allowed_token_set() -> HashSet<String> {
    HashSet::from([String::from(
        "So11111111111111111111111111111111111111112",
    )])
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_calculate_rsi_basic() {
        let mut history = VecDeque::new();
        let start = Utc::now();
        for i in 0..20 {
            history.push_back((start + Duration::minutes(i as i64), 100.0 + i as f64));
        }
        let rsi = calculate_rsi(&history, 14).unwrap();
        assert!(rsi > 50.0);
    }

    #[test]
    fn test_percentage_change_since() {
        let mut history = VecDeque::new();
        let start = Utc::now();
        for i in 0..10 {
            history.push_back((start + Duration::minutes(i as i64 * 5), 100.0 + i as f64));
        }
        let change =
            percentage_change_since(&history, history.back().unwrap().0, Duration::minutes(25))
                .unwrap();
        assert!(change > 0.0);
    }
}
