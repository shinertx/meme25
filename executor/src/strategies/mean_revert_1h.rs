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
pub struct MeanRevert1h {
    period_hours: u32,
    z_score_threshold: f64,
    #[serde(skip)]
    price_history: HashMap<String, VecDeque<(DateTime<Utc>, f64)>>,
    #[serde(skip)]
    positions: HashMap<String, MeanReversionPosition>,
}

#[derive(Debug, Clone)]
struct MeanReversionPosition {
    entry_price: f64,
    entry_time: DateTime<Utc>,
    z_score: f64,
}

#[async_trait]
impl Strategy for MeanRevert1h {
    fn id(&self) -> &'static str {
        "mean_revert_1h"
    }

    fn subscriptions(&self) -> HashSet<EventType> {
        [EventType::Price].iter().cloned().collect()
    }

    async fn init(&mut self, params: &Value) -> Result<()> {
        #[derive(Deserialize)]
        struct Params {
            period_hours: u32,
            z_score_threshold: f64,
        }

        let p: Params = serde_json::from_value(params.clone())?;
        self.period_hours = p.period_hours;
        self.z_score_threshold = p.z_score_threshold;

        info!(
            strategy = self.id(),
            period_hours = self.period_hours,
            z_score_threshold = self.z_score_threshold,
            "Mean reversion strategy initialized"
        );

        Ok(())
    }

    async fn on_event(&mut self, event: &MarketEvent) -> Result<StrategyAction> {
        if let MarketEvent::Price(tick) = event {
            let history = self
                .price_history
                .entry(tick.token_address.clone())
                .or_default();

            // Add new price
            history.push_back((tick.timestamp, tick.price_usd));

            // Remove old data
            let cutoff = tick.timestamp - Duration::hours(self.period_hours as i64);
            while let Some((time, _)) = history.front() {
                if *time < cutoff {
                    history.pop_front();
                } else {
                    break;
                }
            }

            // Need minimum data points
            if history.len() < 20 {
                return Ok(StrategyAction::Hold);
            }

            // Calculate statistics
            let prices: Vec<f64> = history.iter().map(|(_, p)| *p).collect();
            let mean = prices.iter().sum::<f64>() / prices.len() as f64;
            let variance =
                prices.iter().map(|p| (p - mean).powi(2)).sum::<f64>() / prices.len() as f64;
            let std_dev = variance.sqrt();

            if std_dev == 0.0 {
                return Ok(StrategyAction::Hold);
            }

            let z_score = (tick.price_usd - mean) / std_dev;

            // Check for existing position
            if let Some(position) = self.positions.get(&tick.token_address) {
                // Exit conditions
                let price_change_pct = if position.entry_price.abs() > f64::EPSILON {
                    ((tick.price_usd - position.entry_price) / position.entry_price) * 100.0
                } else {
                    0.0
                };

                if (position.z_score > 0.0 && z_score <= 0.0) || // Long exit
                   (position.z_score < 0.0 && z_score >= 0.0) || // Short exit
                   tick.timestamp.signed_duration_since(position.entry_time) > Duration::hours(4)
                {
                    info!(
                        strategy = self.id(),
                        token = %tick.token_address,
                        entry_z_score = position.z_score,
                        entry_price = position.entry_price,
                        current_price = tick.price_usd,
                        price_change_pct = price_change_pct,
                        current_z_score = z_score,
                        "MEAN REVERSION EXIT signal"
                    );

                    self.positions.remove(&tick.token_address);
                    return Ok(StrategyAction::ClosePosition);
                }
            } else {
                // Entry conditions
                if z_score.abs() > self.z_score_threshold && tick.liquidity_usd > 20000.0 {
                    let side = if z_score > 0.0 {
                        Side::Short
                    } else {
                        Side::Long
                    };

                    info!(
                        strategy = self.id(),
                        token = %tick.token_address,
                        z_score = z_score,
                        side = ?side,
                        "MEAN REVERSION ENTRY signal"
                    );

                    self.positions.insert(
                        tick.token_address.clone(),
                        MeanReversionPosition {
                            entry_price: tick.price_usd,
                            entry_time: tick.timestamp,
                            z_score,
                        },
                    );

                    let confidence = (z_score.abs() / self.z_score_threshold * 0.5).min(0.9);

                    let order = OrderDetails {
                        token_address: tick.token_address.clone(),
                        symbol: format!("MEME_{}", &tick.token_address[..6]),
                        suggested_size_usd: 40.0,
                        confidence,
                        side,
                        strategy_metadata: HashMap::from([
                            ("z_score".to_string(), serde_json::json!(z_score)),
                            ("mean_price".to_string(), serde_json::json!(mean)),
                            ("std_dev".to_string(), serde_json::json!(std_dev)),
                        ]),
                        risk_metrics: RiskMetrics {
                            position_size_pct: 0.015,
                            stop_loss_price: if side == Side::Long {
                                Some(tick.price_usd * 0.93) // 7% stop
                            } else {
                                Some(tick.price_usd * 1.07)
                            },
                            take_profit_price: Some(mean), // Target mean reversion
                            max_slippage_bps: 30,
                            time_limit_seconds: Some(600),
                        },
                    };

                    return Ok(StrategyAction::Execute(order));
                }
            }
        }

        Ok(StrategyAction::Hold)
    }

    fn get_state(&self) -> Value {
        serde_json::json!({
            "period_hours": self.period_hours,
            "z_score_threshold": self.z_score_threshold,
            "tracked_tokens": self.price_history.len(),
            "open_positions": self.positions.len(),
        })
    }
}
