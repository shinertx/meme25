use crate::{config::Config, metrics::Metrics};
use anyhow::Result;
use shared_models::{PortfolioRiskMetrics, RiskEventType, RiskLevel, Side, Trade};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

const POSITION_EPSILON: f64 = 0.01;

#[derive(Debug, Clone)]
pub struct RiskLimits {
    pub initial_capital_usd: f64,
    pub max_position_usd: f64,
    pub max_daily_loss_usd: f64,
    pub max_portfolio_usd: f64,
    pub max_strategy_allocation_pct: f64,
}

impl RiskLimits {
    pub fn from_config(config: &Config) -> Self {
        let mut limits = Self {
            initial_capital_usd: config.initial_capital_usd,
            max_position_usd: config.max_portfolio_size_usd
                * (config.max_position_size_percent / 100.0),
            max_daily_loss_usd: config.initial_capital_usd
                * (config.max_daily_drawdown_percent / 100.0),
            max_portfolio_usd: config.max_portfolio_size_usd,
            max_strategy_allocation_pct: env::var("MAX_STRATEGY_ALLOCATION_PCT")
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(10.0),
        };

        if let Ok(value) = env::var("RISK_MAX_POSITION_USD") {
            if let Ok(parsed) = value.parse::<f64>() {
                limits.max_position_usd = parsed;
            }
        }

        if let Ok(value) = env::var("RISK_MAX_DAILY_LOSS_USD") {
            if let Ok(parsed) = value.parse::<f64>() {
                limits.max_daily_loss_usd = parsed;
            }
        }

        if let Ok(value) = env::var("RISK_MAX_PORTFOLIO_USD") {
            if let Ok(parsed) = value.parse::<f64>() {
                limits.max_portfolio_usd = parsed;
            }
        }

        limits
    }

    pub fn max_strategy_exposure_usd(&self) -> f64 {
        (self.max_portfolio_usd * (self.max_strategy_allocation_pct / 100.0)).max(0.0)
    }
}

impl Default for RiskLimits {
    fn default() -> Self {
        Self {
            initial_capital_usd: 200.0,
            max_position_usd: 50.0,
            max_daily_loss_usd: 20.0,
            max_portfolio_usd: 100.0,
            max_strategy_allocation_pct: 10.0,
        }
    }
}

pub struct RiskManager {
    limits: RiskLimits,
    daily_pnl: RwLock<f64>,
    position_sizes: RwLock<HashMap<String, f64>>,
    strategy_exposure: RwLock<HashMap<String, f64>>,
    rejection_counters: RwLock<HashMap<String, HashMap<String, u64>>>,
    metrics: Option<Arc<Metrics>>,
}

impl RiskManager {
    pub fn new() -> Self {
        Self::from_limits(RiskLimits::default())
    }

    pub fn from_limits(limits: RiskLimits) -> Self {
        Self {
            metrics: None,
            limits,
            daily_pnl: RwLock::new(0.0),
            position_sizes: RwLock::new(HashMap::new()),
            strategy_exposure: RwLock::new(HashMap::new()),
            rejection_counters: RwLock::new(HashMap::new()),
        }
    }

    pub fn from_config(config: &Config) -> Self {
        Self::from_limits(RiskLimits::from_config(config))
    }

    pub fn with_metrics(mut self, metrics: Arc<Metrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    pub fn attach_metrics(&mut self, metrics: Arc<Metrics>) {
        self.metrics = Some(metrics);
    }

    pub fn limits(&self) -> &RiskLimits {
        &self.limits
    }

    pub async fn evaluate_trade(&self, trade: &Trade) -> Result<TradeDecision> {
        let daily_pnl = *self.daily_pnl.read().await;
        if daily_pnl <= -self.limits.max_daily_loss_usd {
            let description = format!(
                "Daily loss limit breached: {:.2} ≤ -{:.2}",
                daily_pnl, self.limits.max_daily_loss_usd
            );
            self.record_rejection(
                &trade.strategy_id,
                RiskEventType::DailyLossLimit,
                RiskLevel::High,
                &description,
            )
            .await;
            return Ok(TradeDecision::Reject {
                event_type: RiskEventType::DailyLossLimit,
                severity: RiskLevel::High,
                description,
            });
        }

        let (existing_position, current_total_abs) = {
            let positions = self.position_sizes.read().await;
            let existing = positions.get(&trade.symbol).copied().unwrap_or(0.0);
            let sum_other = positions
                .iter()
                .map(|(symbol, exposure)| {
                    if symbol == &trade.symbol {
                        0.0
                    } else {
                        exposure.abs()
                    }
                })
                .sum::<f64>();
            (existing, sum_other)
        };

        let trade_value = trade.quantity * trade.price;
        let signed_trade_value = match trade.side {
            Side::Long => trade_value,
            Side::Short => -trade_value,
        };

        let proposed_position = existing_position + signed_trade_value;
        if proposed_position.abs() > self.limits.max_position_usd {
            let description = format!(
                "Proposed position {:.2} USD exceeds per-position limit {:.2}",
                proposed_position, self.limits.max_position_usd
            );
            self.record_rejection(
                &trade.strategy_id,
                RiskEventType::PositionSizeExceeded,
                RiskLevel::High,
                &description,
            )
            .await;
            return Ok(TradeDecision::Reject {
                event_type: RiskEventType::PositionSizeExceeded,
                severity: RiskLevel::High,
                description,
            });
        }

        let proposed_total_abs = current_total_abs + proposed_position.abs();
        if proposed_total_abs > self.limits.max_portfolio_usd {
            let description = format!(
                "Portfolio exposure {:.2} USD would exceed cap {:.2}",
                proposed_total_abs, self.limits.max_portfolio_usd
            );
            self.record_rejection(
                &trade.strategy_id,
                RiskEventType::PortfolioExposure,
                RiskLevel::Medium,
                &description,
            )
            .await;
            return Ok(TradeDecision::Reject {
                event_type: RiskEventType::PortfolioExposure,
                severity: RiskLevel::Medium,
                description,
            });
        }

        let strategy_current = {
            let exposures = self.strategy_exposure.read().await;
            exposures.get(&trade.strategy_id).copied().unwrap_or(0.0)
        };
        let proposed_strategy_exposure = (strategy_current + signed_trade_value).abs();
        let strategy_cap = self.limits.max_strategy_exposure_usd();
        if strategy_cap > 0.0 && proposed_strategy_exposure > strategy_cap {
            let description = format!(
                "Strategy {} exposure {:.2} USD exceeds {:.2} cap",
                trade.strategy_id, proposed_strategy_exposure, strategy_cap
            );
            self.record_rejection(
                &trade.strategy_id,
                RiskEventType::PortfolioExposure,
                RiskLevel::Medium,
                &description,
            )
            .await;
            return Ok(TradeDecision::Reject {
                event_type: RiskEventType::PortfolioExposure,
                severity: RiskLevel::Medium,
                description,
            });
        }

        let utilization = if self.limits.max_portfolio_usd > 0.0 {
            proposed_total_abs / self.limits.max_portfolio_usd
        } else {
            0.0
        };

        if utilization > 0.9 {
            warn!(
                strategy = %trade.strategy_id,
                symbol = %trade.symbol,
                utilization = %format!("{:.2}", utilization * 100.0),
                "Portfolio utilization above 90% pre-trade"
            );
        } else {
            debug!(
                strategy = %trade.strategy_id,
                symbol = %trade.symbol,
                position_usd = proposed_position,
                utilization_pct = %format!("{:.2}", utilization * 100.0),
                "Trade cleared risk checks"
            );
        }

        Ok(TradeDecision::Allow)
    }

    pub async fn update_position(&self, trade: &Trade) -> Result<()> {
        let trade_value = trade.quantity * trade.price;
        let signed_trade_value = match trade.side {
            Side::Long => trade_value,
            Side::Short => -trade_value,
        };

        let (total_exposure, position_count) = {
            let mut positions = self.position_sizes.write().await;
            let entry = positions.entry(trade.symbol.clone()).or_insert(0.0);
            *entry += signed_trade_value;
            if entry.abs() < POSITION_EPSILON {
                positions.remove(&trade.symbol);
            }
            let total = positions
                .values()
                .map(|exposure| exposure.abs())
                .sum::<f64>();
            let count = positions.len() as u32;
            (total, count)
        };

        {
            let mut strategy_exposure = self.strategy_exposure.write().await;
            let entry = strategy_exposure
                .entry(trade.strategy_id.clone())
                .or_insert(0.0);
            *entry += signed_trade_value;
            if entry.abs() < POSITION_EPSILON {
                strategy_exposure.remove(&trade.strategy_id);
            }
        }

        if let Some(metrics) = &self.metrics {
            if self.limits.max_portfolio_usd > 0.0 {
                let utilization_pct = (total_exposure / self.limits.max_portfolio_usd) * 100.0;
                metrics.update_exposure_utilization(utilization_pct);
            }
            let current_pnl = *self.daily_pnl.read().await;
            metrics.update_portfolio_value(self.limits.initial_capital_usd + current_pnl);
            metrics.update_daily_pnl(current_pnl);
        }

        debug!(
            strategy = %trade.strategy_id,
            symbol = %trade.symbol,
            side = ?trade.side,
            notional_usd = trade_value,
            net_positions = position_count,
            "Position book updated"
        );

        Ok(())
    }

    pub async fn update_daily_pnl(&self, pnl_change: f64) {
        let mut daily_pnl = self.daily_pnl.write().await;
        *daily_pnl += pnl_change;
        let current = *daily_pnl;
        drop(daily_pnl);

        if let Some(metrics) = &self.metrics {
            metrics.update_daily_pnl(current);
            metrics.update_portfolio_value(self.limits.initial_capital_usd + current);
        }

        let warning_threshold = -self.limits.max_daily_loss_usd * 0.8;
        if current <= -self.limits.max_daily_loss_usd {
            warn!(
                current_daily = %format!("{:.2}", current),
                max_loss = %format!("-{:.2}", self.limits.max_daily_loss_usd),
                "Daily loss limit breached"
            );
        } else if current <= warning_threshold {
            warn!(
                current_daily = %format!("{:.2}", current),
                max_loss = %format!("-{:.2}", self.limits.max_daily_loss_usd),
                "Daily loss limit approaching"
            );
        }
    }

    pub async fn get_risk_metrics(&self) -> PortfolioRiskMetrics {
        let daily_pnl = *self.daily_pnl.read().await;
        let positions = self.position_sizes.read().await;
        let current_exposure: f64 = positions.values().map(|exposure| exposure.abs()).sum();
        let position_count = positions.len() as u32;
        drop(positions);

        let portfolio_utilization = if self.limits.max_portfolio_usd > 0.0 {
            current_exposure / self.limits.max_portfolio_usd
        } else {
            0.0
        };

        PortfolioRiskMetrics {
            portfolio_value: self.limits.initial_capital_usd + daily_pnl,
            daily_pnl,
            max_drawdown: daily_pnl.min(0.0),
            exposure_percentage: portfolio_utilization * 100.0,
            var_95: daily_pnl * 1.65,
            position_count,
            risk_score: if portfolio_utilization > 0.9 {
                RiskLevel::High
            } else if portfolio_utilization > 0.65 {
                RiskLevel::Medium
            } else {
                RiskLevel::Low
            },
        }
    }

    pub async fn reset_daily_metrics(&self) {
        let mut daily_pnl = self.daily_pnl.write().await;
        *daily_pnl = 0.0;
        drop(daily_pnl);

        if let Some(metrics) = &self.metrics {
            metrics.update_daily_pnl(0.0);
            metrics.update_portfolio_value(self.limits.initial_capital_usd);
        }

        info!("📊 Daily risk metrics reset");
    }

    pub async fn position_snapshot(&self) -> HashMap<String, f64> {
        self.position_sizes.read().await.clone()
    }

    pub async fn strategy_exposure_snapshot(&self) -> HashMap<String, f64> {
        self.strategy_exposure.read().await.clone()
    }

    pub async fn rejection_snapshot(&self) -> HashMap<String, HashMap<String, u64>> {
        self.rejection_counters.read().await.clone()
    }

    async fn record_rejection(
        &self,
        strategy_id: &str,
        event_type: RiskEventType,
        severity: RiskLevel,
        description: &str,
    ) {
        if let Some(metrics) = &self.metrics {
            metrics.record_risk_event(strategy_id, event_type.to_string());
        }

        let mut counters = self.rejection_counters.write().await;
        let strategy_entry = counters
            .entry(strategy_id.to_string())
            .or_insert_with(HashMap::new);
        let reason = event_type.to_string().to_owned();
        *strategy_entry.entry(reason).or_insert(0) += 1;

        warn!(
            strategy = %strategy_id,
            event = %event_type.to_string(),
            severity = ?severity,
            "Trade rejected: {description}"
        );
    }
}

impl Default for RiskManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of evaluating a trade against the risk thresholds.
#[derive(Debug, Clone)]
pub enum TradeDecision {
    Allow,
    Reject {
        event_type: RiskEventType,
        severity: RiskLevel,
        description: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_trade(symbol: &str, side: Side, quantity: f64, price: f64) -> Trade {
        Trade {
            id: "t1".into(),
            strategy_id: "strat".into(),
            symbol: symbol.into(),
            side,
            quantity,
            price,
            timestamp: Utc::now(),
            profit_loss: 0.0,
        }
    }

    #[tokio::test]
    async fn retains_short_positions_until_flushed() {
        let manager = RiskManager::new();
        let short_trade = sample_trade("SOL", Side::Short, 1.0, 10.0);
        manager.update_position(&short_trade).await.unwrap();
        assert_eq!(
            manager.position_snapshot().await.get("SOL").copied(),
            Some(-10.0)
        );

        let covering_trade = sample_trade("SOL", Side::Long, 1.0, 10.0);
        manager.update_position(&covering_trade).await.unwrap();
        assert!(manager.position_snapshot().await.get("SOL").is_none());
    }

    #[tokio::test]
    async fn rejects_trade_when_daily_loss_limit_breached() {
        let manager = RiskManager::new();
        manager.update_daily_pnl(-25.0).await;
        let trade = sample_trade("SOL", Side::Long, 1.0, 10.0);
        let decision = manager.evaluate_trade(&trade).await.unwrap();

        assert!(matches!(
            decision,
            TradeDecision::Reject {
                event_type: RiskEventType::DailyLossLimit,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn rejects_trade_when_position_limit_exceeded() {
        let mut limits = RiskLimits::default();
        limits.max_strategy_allocation_pct = 100.0;
        limits.max_portfolio_usd = 1_000.0;
        let manager = RiskManager::from_limits(limits);
        let trade_ok = sample_trade("SOL", Side::Long, 0.5, 50.0);
        assert!(matches!(
            manager.evaluate_trade(&trade_ok).await.unwrap(),
            TradeDecision::Allow
        ));

        manager.update_position(&trade_ok).await.unwrap();

        let trade_excess = sample_trade("SOL", Side::Long, 1.0, 60.0);
        assert!(matches!(
            manager.evaluate_trade(&trade_excess).await.unwrap(),
            TradeDecision::Reject {
                event_type: RiskEventType::PositionSizeExceeded,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn rejects_trade_when_strategy_allocation_exceeded() {
        let mut limits = RiskLimits::default();
        limits.max_portfolio_usd = 1_000.0;
        limits.max_position_usd = 1_000.0;
        limits.max_strategy_allocation_pct = 10.0;

        let manager = RiskManager::from_limits(limits);

        let opening_trade = Trade {
            strategy_id: "momentum".into(),
            ..sample_trade("SOL", Side::Long, 5.0, 10.0)
        };
        manager.update_position(&opening_trade).await.unwrap();

        let oversized = Trade {
            strategy_id: "momentum".into(),
            ..sample_trade("SOL", Side::Long, 10.0, 15.0)
        };

        let decision = manager.evaluate_trade(&oversized).await.unwrap();
        assert!(matches!(
            decision,
            TradeDecision::Reject {
                event_type: RiskEventType::PortfolioExposure,
                ..
            }
        ));
    }
}
