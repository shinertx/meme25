use crate::strategies::{EventType, MarketEvent, OrderDetails, Strategy, StrategyAction};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared_models::{RiskMetrics, Side};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use tracing::{info, warn};

const FIVE_MINUTES: i64 = 5;
const BARS_PER_HOUR: usize = 12;

#[derive(Debug, Clone)]
struct FiveMinuteBar {
    bucket_start: DateTime<Utc>,
    close: f64,
    volume: f64,
}

#[derive(Debug, Clone, Copy)]
struct SignalMetrics {
    last_price: f64,
    prev_high: f64,
    avg_volume: f64,
    slope: f64,
    breakout_strength: f64,
    volume_ratio: f64,
    slope_ratio: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EthBreakoutMomentum {
    #[serde(default = "default_lookback_hours")]
    lookback_hours: usize,
    #[serde(default = "default_stop_loss_pct")]
    stop_loss_pct: f64,
    #[serde(default = "default_take_profit_pct")]
    take_profit_pct: f64,
    #[serde(default = "default_breakout_buffer")]
    breakout_buffer: f64,
    #[serde(default = "default_volume_multiplier")]
    volume_multiplier: f64,
    #[serde(default = "default_slope_min")]
    slope_min: f64,
    #[serde(default = "default_cooldown_minutes")]
    cooldown_minutes: i64,
    #[serde(default = "default_min_liquidity_usd")]
    min_liquidity_usd: f64,
    #[serde(default = "default_session_start_hour")]
    session_start_hour: u32,
    #[serde(default = "default_session_end_hour")]
    session_end_hour: u32,
    #[serde(default = "default_max_hold_minutes")]
    max_hold_minutes: u64,
    #[serde(default = "default_suggested_size_usd")]
    suggested_size_usd: f64,
    #[serde(default)]
    allowed_tokens: Option<HashSet<String>>,
    #[serde(skip)]
    histories: HashMap<String, VecDeque<FiveMinuteBar>>,
    #[serde(skip)]
    last_signal: HashMap<String, DateTime<Utc>>,
    #[serde(skip)]
    signals_emitted: u64,
}

impl Default for EthBreakoutMomentum {
    fn default() -> Self {
        Self {
            lookback_hours: default_lookback_hours(),
            stop_loss_pct: default_stop_loss_pct(),
            take_profit_pct: default_take_profit_pct(),
            breakout_buffer: default_breakout_buffer(),
            volume_multiplier: default_volume_multiplier(),
            slope_min: default_slope_min(),
            cooldown_minutes: default_cooldown_minutes(),
            min_liquidity_usd: default_min_liquidity_usd(),
            session_start_hour: default_session_start_hour(),
            session_end_hour: default_session_end_hour(),
            max_hold_minutes: default_max_hold_minutes(),
            suggested_size_usd: default_suggested_size_usd(),
            allowed_tokens: None,
            histories: HashMap::new(),
            last_signal: HashMap::new(),
            signals_emitted: 0,
        }
    }
}

fn default_lookback_hours() -> usize {
    24
}

fn default_stop_loss_pct() -> f64 {
    0.012
}

fn default_take_profit_pct() -> f64 {
    0.017
}

fn default_breakout_buffer() -> f64 {
    0.0025
}

fn default_volume_multiplier() -> f64 {
    2.0
}

fn default_slope_min() -> f64 {
    0.0003
}

fn default_cooldown_minutes() -> i64 {
    120
}

fn default_min_liquidity_usd() -> f64 {
    5_000_000.0
}

fn default_session_start_hour() -> u32 {
    13 // 9am ET
}

fn default_session_end_hour() -> u32 {
    23 // 7pm ET
}

fn default_max_hold_minutes() -> u64 {
    300 // 5 hours
}

fn default_suggested_size_usd() -> f64 {
    4.0 // 2% of $200 starting stack
}

#[async_trait]
impl Strategy for EthBreakoutMomentum {
    fn id(&self) -> &'static str {
        "eth_breakout_momentum"
    }

    fn subscriptions(&self) -> HashSet<EventType> {
        [EventType::Price].iter().cloned().collect()
    }

    async fn init(&mut self, params: &Value) -> Result<()> {
        #[derive(Debug, Deserialize)]
        struct Params {
            #[serde(default = "default_lookback_hours")]
            lookback_hours: usize,
            #[serde(default = "default_stop_loss_pct")]
            stop_loss_pct: f64,
            #[serde(default = "default_take_profit_pct")]
            take_profit_pct: f64,
            #[serde(default = "default_breakout_buffer")]
            breakout_buffer: f64,
            #[serde(default = "default_volume_multiplier")]
            volume_multiplier: f64,
            #[serde(default = "default_slope_min")]
            slope_min: f64,
            #[serde(default = "default_cooldown_minutes")]
            cooldown_minutes: i64,
            #[serde(default = "default_min_liquidity_usd")]
            min_liquidity_usd: f64,
            #[serde(default = "default_session_start_hour")]
            session_start_hour: u32,
            #[serde(default = "default_session_end_hour")]
            session_end_hour: u32,
            #[serde(default = "default_max_hold_minutes")]
            max_hold_minutes: u64,
            #[serde(default = "default_suggested_size_usd")]
            suggested_size_usd: f64,
            #[serde(default)]
            allowed_tokens: Option<Vec<String>>,
        }

        let parsed: Params = serde_json::from_value(params.clone())
            .map_err(|err| anyhow!("invalid params: {}", err))?;

        self.lookback_hours = parsed.lookback_hours.max(6); // guard for minimum data
        self.stop_loss_pct = parsed.stop_loss_pct;
        self.take_profit_pct = parsed.take_profit_pct.max(self.stop_loss_pct + 0.001);
        self.breakout_buffer = parsed.breakout_buffer.max(0.0005);
        self.volume_multiplier = parsed.volume_multiplier.max(1.2);
        self.slope_min = parsed.slope_min.max(0.0001);
        self.cooldown_minutes = parsed.cooldown_minutes.max(30);
        self.min_liquidity_usd = parsed.min_liquidity_usd;
        self.session_start_hour = parsed.session_start_hour.min(23);
        self.session_end_hour = parsed.session_end_hour.min(24);
        self.max_hold_minutes = parsed.max_hold_minutes.max(60);
        self.suggested_size_usd = parsed.suggested_size_usd.max(1.0);
        self.allowed_tokens = parsed
            .allowed_tokens
            .map(|tokens| tokens.into_iter().collect::<HashSet<_>>());

        self.histories.clear();
        self.last_signal.clear();
        self.signals_emitted = 0;

        info!(
            strategy = self.id(),
            lookback_hours = self.lookback_hours,
            stop_loss_pct = format!("{:.4}", self.stop_loss_pct),
            take_profit_pct = format!("{:.4}", self.take_profit_pct),
            breakout_buffer = format!("{:.4}", self.breakout_buffer),
            volume_multiplier = format!("{:.2}", self.volume_multiplier),
            slope_min = format!("{:.5}", self.slope_min),
            cooldown_minutes = self.cooldown_minutes,
            min_liquidity = format!("{:.0}", self.min_liquidity_usd),
            session_start = self.session_start_hour,
            session_end = self.session_end_hour,
            max_hold_minutes = self.max_hold_minutes,
            suggested_size_usd = format!("{:.2}", self.suggested_size_usd),
            allowed_tokens = self
                .allowed_tokens
                .as_ref()
                .map(|set| set.len())
                .unwrap_or(0),
            "ETH breakout momentum strategy initialized"
        );

        Ok(())
    }

    async fn on_event(&mut self, event: &MarketEvent) -> Result<StrategyAction> {
        let MarketEvent::Price(tick) = event else {
            return Ok(StrategyAction::Hold);
        };

        if let Some(allowed) = &self.allowed_tokens {
            if !allowed.contains(&tick.token_address) {
                return Ok(StrategyAction::Hold);
            }
        }

        if tick.liquidity_usd < self.min_liquidity_usd {
            return Ok(StrategyAction::Hold);
        }

        if !in_session(
            tick.timestamp,
            self.session_start_hour,
            self.session_end_hour,
        ) {
            return Ok(StrategyAction::Hold);
        }

        // Capture strategy ID before mutable borrow of histories
        let strategy_id = self.id();
        let lookback_hours = self.lookback_hours;

        let history = self
            .histories
            .entry(tick.token_address.clone())
            .or_insert_with(VecDeque::new);

        let bucket_start = floor_to_interval(tick.timestamp, FIVE_MINUTES);

        if let Some(last) = history.back_mut() {
            match bucket_start.cmp(&last.bucket_start) {
                Ordering::Less => {
                    // Skip late data, but warn once per token
                    warn!(
                        strategy = strategy_id,
                        token = %tick.token_address,
                        event_time = ?tick.timestamp,
                        bucket_time = ?bucket_start,
                        "Out-of-order price tick encountered"
                    );
                }
                Ordering::Equal => {
                    last.close = tick.price_usd;
                    last.volume = tick.volume_usd_5m.max(tick.volume_usd_1m);
                    last.bucket_start = bucket_start;
                }
                Ordering::Greater => {
                    history.push_back(FiveMinuteBar {
                        bucket_start,
                        close: tick.price_usd,
                        volume: tick.volume_usd_5m.max(tick.volume_usd_1m),
                    });
                }
            }
        } else {
            history.push_back(FiveMinuteBar {
                bucket_start,
                close: tick.price_usd,
                volume: tick.volume_usd_5m.max(tick.volume_usd_1m),
            });
        }

        trim_history(history, lookback_hours);

        let lookback_bars = lookback_hours * BARS_PER_HOUR;
        if history.len() <= lookback_bars {
            return Ok(StrategyAction::Hold);
        }

        let last_bar = match history.back() {
            Some(bar) => bar,
            None => return Ok(StrategyAction::Hold),
        };

        // Cooldown enforcement
        if let Some(last_time) = self.last_signal.get(&tick.token_address) {
            if tick
                .timestamp
                .signed_duration_since(*last_time)
                .num_minutes()
                < self.cooldown_minutes
            {
                return Ok(StrategyAction::Hold);
            }
        }

        let last_index = history.len() - 1;
        let start_index = last_index.saturating_sub(lookback_bars);

        if start_index >= last_index {
            return Ok(StrategyAction::Hold);
        }

        let mut prev_high = f64::MIN;
        let mut volume_sum = 0.0;
        let mut volume_count = 0usize;
        let mut slope_baseline: Option<f64> = None;

        for (idx, bar) in history.iter().enumerate() {
            if idx < start_index || idx >= last_index {
                continue;
            }

            if slope_baseline.is_none() && bar.close.is_finite() && bar.close > 0.0 {
                slope_baseline = Some(bar.close);
            }

            if bar.close.is_finite() {
                prev_high = prev_high.max(bar.close);
            }

            if bar.volume.is_finite() {
                volume_sum += bar.volume;
                volume_count += 1;
            }
        }

        if !prev_high.is_finite() || prev_high <= 0.0 || volume_count == 0 {
            return Ok(StrategyAction::Hold);
        }

        let avg_volume = volume_sum / volume_count as f64;

        if avg_volume <= 0.0 {
            return Ok(StrategyAction::Hold);
        }

        let slope_baseline = slope_baseline.unwrap_or(prev_high);

        let slope = (last_bar.close - slope_baseline) / slope_baseline;

        let breakout_condition =
            last_bar.close > prev_high * (1.0 + self.breakout_buffer + f64::EPSILON);
        let volume_condition = last_bar.volume > avg_volume * self.volume_multiplier;
        let slope_condition = slope >= self.slope_min;

        if breakout_condition && volume_condition && slope_condition {
            let breakout_strength = (last_bar.close / prev_high - 1.0).max(0.0);
            let volume_ratio = last_bar.volume / avg_volume;
            let slope_ratio = if self.slope_min > 0.0 {
                (slope / self.slope_min).max(0.0)
            } else {
                1.0
            };

            let confidence = compute_confidence(
                breakout_strength,
                self.breakout_buffer,
                volume_ratio,
                slope_ratio,
            );

            let symbol_suffix: String = tick.token_address.chars().take(6).collect();
            let order = OrderDetails {
                token_address: tick.token_address.clone(),
                symbol: format!("CBETH_{}", symbol_suffix),
                suggested_size_usd: self.suggested_size_usd,
                confidence,
                side: Side::Long,
                strategy_metadata: HashMap::from([
                    (
                        "breakout_strength".to_string(),
                        serde_json::json!(breakout_strength),
                    ),
                    ("volume_ratio".to_string(), serde_json::json!(volume_ratio)),
                    ("slope".to_string(), serde_json::json!(slope)),
                    ("prev_high".to_string(), serde_json::json!(prev_high)),
                    ("avg_volume".to_string(), serde_json::json!(avg_volume)),
                ]),
                risk_metrics: RiskMetrics {
                    position_size_pct: 0.02,
                    stop_loss_price: Some(last_bar.close * (1.0 - self.stop_loss_pct)),
                    take_profit_price: Some(last_bar.close * (1.0 + self.take_profit_pct)),
                    max_slippage_bps: 25,
                    time_limit_seconds: Some(self.max_hold_minutes * 60),
                },
            };

            info!(
                strategy = strategy_id,
                token = %tick.token_address,
                price = last_bar.close,
                prev_high,
                breakout = format!("{:.4}%", breakout_strength * 100.0),
                volume_ratio = format!("{:.2}", volume_ratio),
                slope = format!("{:.4}%", slope * 100.0),
                confidence = format!("{:.2}", confidence),
                "ETH breakout BUY signal"
            );

            self.last_signal
                .insert(tick.token_address.clone(), tick.timestamp);
            self.signals_emitted += 1;

            return Ok(StrategyAction::Execute(order));
        }

        Ok(StrategyAction::Hold)
    }

    fn get_state(&self) -> Value {
        serde_json::json!({
            "lookback_hours": self.lookback_hours,
            "stop_loss_pct": self.stop_loss_pct,
            "take_profit_pct": self.take_profit_pct,
            "breakout_buffer": self.breakout_buffer,
            "volume_multiplier": self.volume_multiplier,
            "slope_min": self.slope_min,
            "cooldown_minutes": self.cooldown_minutes,
            "min_liquidity_usd": self.min_liquidity_usd,
            "session_start_hour": self.session_start_hour,
            "session_end_hour": self.session_end_hour,
            "max_hold_minutes": self.max_hold_minutes,
            "suggested_size_usd": self.suggested_size_usd,
            "allowed_tokens": self.allowed_tokens,
            "tracked_tokens": self.histories.len(),
            "signals_emitted": self.signals_emitted,
        })
    }
}

fn floor_to_interval(timestamp: DateTime<Utc>, minutes: i64) -> DateTime<Utc> {
    let minute = timestamp.minute() as i64;
    let floored_minute = minute - (minute % minutes);
    timestamp
        .with_minute(floored_minute as u32)
        .and_then(|dt| dt.with_second(0))
        .and_then(|dt| dt.with_nanosecond(0))
        .unwrap_or(timestamp)
}

fn trim_history(history: &mut VecDeque<FiveMinuteBar>, lookback_hours: usize) {
    let max_bars = lookback_hours * BARS_PER_HOUR + BARS_PER_HOUR * 2;
    while history.len() > max_bars {
        history.pop_front();
    }
}

fn update_history(
    history: &mut VecDeque<FiveMinuteBar>,
    bucket_start: DateTime<Utc>,
    price: f64,
    volume_5m: f64,
    volume_1m: f64,
    strategy_id: &str,
    token: &str,
) {
    let volume = volume_5m.max(volume_1m);
    if let Some(last) = history.back_mut() {
        match bucket_start.cmp(&last.bucket_start) {
            Ordering::Less => {
                warn!(
                    strategy = strategy_id,
                    token,
                    event_bucket = ?bucket_start,
                    last_bucket = ?last.bucket_start,
                    "Out-of-order price tick encountered"
                );
            }
            Ordering::Equal => {
                last.close = price;
                last.volume = volume;
                last.bucket_start = bucket_start;
            }
            Ordering::Greater => {
                history.push_back(FiveMinuteBar {
                    bucket_start,
                    close: price,
                    volume,
                });
            }
        }
    } else {
        history.push_back(FiveMinuteBar {
            bucket_start,
            close: price,
            volume,
        });
    }
}

fn compute_metrics(
    history: &VecDeque<FiveMinuteBar>,
    lookback_hours: usize,
    breakout_buffer: f64,
    volume_multiplier: f64,
    slope_min: f64,
) -> Option<SignalMetrics> {
    let lookback_bars = lookback_hours * BARS_PER_HOUR;
    if history.len() <= lookback_bars {
        return None;
    }

    let last_bar = history.back()?;
    let last_index = history.len() - 1;
    let start_index = last_index.saturating_sub(lookback_bars);
    if start_index >= last_index {
        return None;
    }

    let mut prev_high = f64::MIN;
    let mut volume_sum = 0.0;
    let mut volume_count: usize = 0;
    let mut slope_baseline: Option<f64> = None;

    for (idx, bar) in history.iter().enumerate() {
        if idx < start_index || idx >= last_index {
            continue;
        }

        if slope_baseline.is_none() && bar.close.is_finite() && bar.close > 0.0 {
            slope_baseline = Some(bar.close);
        }

        if bar.close.is_finite() {
            prev_high = prev_high.max(bar.close);
        }

        if bar.volume.is_finite() {
            volume_sum += bar.volume;
            volume_count += 1;
        }
    }

    if !prev_high.is_finite() || prev_high <= 0.0 || volume_count == 0 {
        return None;
    }

    let avg_volume = volume_sum / volume_count as f64;
    if avg_volume <= 0.0 {
        return None;
    }

    let slope_baseline = slope_baseline.unwrap_or(prev_high);
    if slope_baseline <= 0.0 {
        return None;
    }

    let slope = (last_bar.close - slope_baseline) / slope_baseline;
    let volume_ratio = last_bar.volume / avg_volume;

    let breakout_condition =
        last_bar.close > prev_high * (1.0 + breakout_buffer + f64::EPSILON);
    let volume_condition = last_bar.volume > avg_volume * volume_multiplier;
    let slope_condition = slope >= slope_min;

    if breakout_condition && volume_condition && slope_condition {
        let breakout_strength = (last_bar.close / prev_high - 1.0).max(0.0);
        let slope_ratio = if slope_min > 0.0 {
            (slope / slope_min).max(0.0)
        } else {
            1.0
        };

        Some(SignalMetrics {
            last_price: last_bar.close,
            prev_high,
            avg_volume,
            slope,
            breakout_strength,
            volume_ratio,
            slope_ratio,
        })
    } else {
        None
    }
}

fn in_session(timestamp: DateTime<Utc>, start_hour: u32, end_hour: u32) -> bool {
    let hour = timestamp.hour();
    if start_hour <= end_hour {
        hour >= start_hour && hour < end_hour
    } else {
        hour >= start_hour || hour < end_hour
    }
}

fn compute_confidence(
    breakout_strength: f64,
    breakout_buffer: f64,
    volume_ratio: f64,
    slope_ratio: f64,
) -> f64 {
    let breakout_term = if breakout_buffer > 0.0 {
        (breakout_strength / breakout_buffer).clamp(0.0, 2.0) * 0.2
    } else {
        0.0
    };
    let volume_term = volume_ratio.clamp(0.0, 3.0) / 3.0 * 0.2;
    let slope_term = slope_ratio.clamp(0.0, 2.0) / 2.0 * 0.1;

    (0.55 + breakout_term + volume_term + slope_term).min(0.95)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn floor_aligns_to_interval() {
        let ts = Utc.with_ymd_and_hms(2025, 10, 13, 14, 17, 42).unwrap();
        let floored = floor_to_interval(ts, FIVE_MINUTES);
        assert_eq!(floored.minute(), 15);
        assert_eq!(floored.second(), 0);
    }

    #[test]
    fn session_window_handles_wraparound() {
        let ts = Utc.with_ymd_and_hms(2025, 10, 13, 2, 0, 0).unwrap();
        assert!(in_session(ts, 22, 3));
        assert!(!in_session(ts, 9, 17));
    }

    #[test]
    fn confidence_respects_caps() {
        let confidence = compute_confidence(0.01, 0.0025, 5.0, 3.0);
        assert!(confidence <= 0.95);
        assert!(confidence >= 0.55);
    }
}
