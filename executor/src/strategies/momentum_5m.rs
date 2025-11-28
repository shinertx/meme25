use crate::strategies::{EventType, MarketEvent, OrderDetails, Strategy, StrategyAction};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared_models::{RiskMetrics, Side};
use std::collections::{HashMap, HashSet, VecDeque};
use tracing::{debug, info, warn};

/// Debug mode relaxed thresholds for testing signal generation
/// These are intentionally lower than production thresholds to force trade signals
const DEBUG_MIN_PRICE_CHANGE: f64 = 0.01;        // 1% minimum price change (vs typical 5%)
const DEBUG_MIN_VOL_RATIO: f64 = 1.0;            // Any volume increase (vs typical 2x)
const DEBUG_MIN_LIQUIDITY_USD: f64 = 1000.0;     // Minimal liquidity (vs typical $50k)

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Momentum5m {
    lookback: usize,
    vol_multiplier: f64,
    price_change_threshold: f64,
    regime_sma_lookback: usize,
    cooldown_minutes: i64,
    min_liquidity_usd: f64,
    allowed_tokens: Option<HashSet<String>>,
    /// Debug mode: when true, bypasses thresholds and logs detailed signal analysis
    #[serde(default)]
    debug_mode: bool,
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
            #[serde(default)]
            debug_mode: bool,
        }

        let p: Params = serde_json::from_value(params.clone())?;
        self.lookback = p.lookback;
        self.vol_multiplier = p.vol_multiplier;
        self.price_change_threshold = p.price_change_threshold;
        self.regime_sma_lookback = p.regime_sma_lookback;
        self.cooldown_minutes = p.cooldown_minutes;
        self.min_liquidity_usd = p.min_liquidity_usd;
        self.allowed_tokens = p.allowed_tokens.map(|tokens| tokens.into_iter().collect());
        self.debug_mode = p.debug_mode;

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
            debug_mode = self.debug_mode,
            allow_count,
            "Momentum strategy initialized"
        );
        
        if self.debug_mode {
            warn!(
                strategy = self.id(),
                "⚠️  DEBUG MODE ENABLED - Strategy will log detailed signal analysis and may force trades"
            );
        }

        Ok(())
    }

    async fn on_event(&mut self, event: &MarketEvent) -> Result<StrategyAction> {
        // Capture strategy ID upfront to avoid borrow conflicts
        let strategy_id = self.id();
        let debug_mode = self.debug_mode;
        let lookback = self.lookback;
        let cooldown_minutes = self.cooldown_minutes;
        let regime_sma_lookback = self.regime_sma_lookback;
        let price_change_threshold = self.price_change_threshold;
        let vol_multiplier = self.vol_multiplier;
        let min_liquidity_usd = self.min_liquidity_usd;
        
        match event {
            MarketEvent::Price(tick) => {
                if let Some(allowed) = &self.allowed_tokens {
                    if !allowed.contains(&tick.token_address) {
                        return Ok(StrategyAction::Hold);
                    }
                }

                // Update history and compute signals
                let history = self
                    .price_history
                    .entry(tick.token_address.clone())
                    .or_insert_with(|| VecDeque::with_capacity(lookback));

                // Add new data point
                history.push_back((tick.timestamp, tick.price_usd, tick.volume_usd_5m));

                // Keep only lookback period
                while history.len() > lookback {
                    history.pop_front();
                }

                // Need full history for signal
                let history_len = history.len();
                if history_len < lookback {
                    if debug_mode {
                        debug!(
                            strategy = strategy_id,
                            token = %tick.token_address.chars().take(8).collect::<String>(),
                            history_len = history_len,
                            lookback = lookback,
                            "[DEBUG] Insufficient history - waiting for more data"
                        );
                    }
                    return Ok(StrategyAction::Hold);
                }

                // Compute signal values from history
                let oldest = match history.front() {
                    Some(data) => *data,
                    None => return Ok(StrategyAction::Hold),
                };
                let newest = match history.back() {
                    Some(data) => *data,
                    None => return Ok(StrategyAction::Hold),
                };

                let price_change = if oldest.1 > 0.0 { (newest.1 - oldest.1) / oldest.1 } else { 0.0 };
                let avg_volume: f64 =
                    history.iter().map(|(_, _, v)| v).sum::<f64>() / history.len() as f64;
                let current_vol_ratio = if avg_volume > 0.0 { newest.2 / avg_volume } else { 0.0 };
                let regime_pass = regime_ok(history, regime_sma_lookback);

                // Check for cooldown
                if let Some(last_time) = self.last_signal.get(&tick.token_address) {
                    if tick.timestamp.signed_duration_since(*last_time)
                        < Duration::minutes(cooldown_minutes)
                    {
                        if debug_mode {
                            debug!(
                                strategy = strategy_id,
                                token = %tick.token_address.chars().take(8).collect::<String>(),
                                cooldown_remaining_min = (Duration::minutes(cooldown_minutes) - tick.timestamp.signed_duration_since(*last_time)).num_minutes(),
                                "[DEBUG] In cooldown period"
                            );
                        }
                        return Ok(StrategyAction::Hold);
                    }
                }

                // [DEBUG MODE] Log all signal components for diagnosis
                if debug_mode {
                    let price_ok = price_change > price_change_threshold;
                    let vol_ok = current_vol_ratio > vol_multiplier;
                    let liq_ok = tick.liquidity_usd > min_liquidity_usd;
                    
                    info!(
                        strategy = strategy_id,
                        token = %tick.token_address.chars().take(8).collect::<String>(),
                        price_usd = tick.price_usd,
                        price_change_pct = format!("{:.2}%", price_change * 100.0),
                        price_threshold_pct = format!("{:.2}%", price_change_threshold * 100.0),
                        price_ok = price_ok,
                        vol_ratio = format!("{:.2}x", current_vol_ratio),
                        vol_threshold = format!("{:.2}x", vol_multiplier),
                        vol_ok = vol_ok,
                        liquidity = format!("${:.0}", tick.liquidity_usd),
                        liq_threshold = format!("${:.0}", min_liquidity_usd),
                        liq_ok = liq_ok,
                        regime_ok = regime_pass,
                        signal_would_fire = price_ok && vol_ok && liq_ok && regime_pass,
                        "[DEBUG] 📊 Signal analysis"
                    );
                }

                // Momentum signal: price increase + volume surge + regime gate
                let signal_fires = price_change > price_change_threshold
                    && current_vol_ratio > vol_multiplier
                    && tick.liquidity_usd > min_liquidity_usd
                    && regime_pass;
                
                // In debug_mode, also fire on relaxed conditions for testing
                let force_signal = debug_mode 
                    && price_change > DEBUG_MIN_PRICE_CHANGE
                    && current_vol_ratio > DEBUG_MIN_VOL_RATIO
                    && tick.liquidity_usd > DEBUG_MIN_LIQUIDITY_USD;
                
                if signal_fires || force_signal {
                    if force_signal && !signal_fires {
                        warn!(
                            strategy = strategy_id,
                            token = %tick.token_address,
                            "[DEBUG] ⚠️  FORCED SIGNAL - would not fire under normal conditions"
                        );
                    }

                    info!(
                        strategy = strategy_id,
                        token = %tick.token_address,
                        price_change = format!("{:.2}%", price_change * 100.0),
                        volume_ratio = format!("{:.2}x", current_vol_ratio),
                        "MOMENTUM BUY signal detected"
                    );

                    self.last_signal
                        .insert(tick.token_address.clone(), tick.timestamp);

                    // Dynamic confidence based on signal strength
                    let confidence = (0.5
                        + (price_change / price_change_threshold * 0.25)
                        + (current_vol_ratio / vol_multiplier * 0.25))
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
            "debug_mode": self.debug_mode,
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
