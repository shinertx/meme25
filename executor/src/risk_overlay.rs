use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use shared_models::error::Result;
use shared_models::{RiskMetrics, Side};
use std::collections::{HashMap, VecDeque};
use tracing::{debug, info, warn};

/// Comprehensive risk overlay system with multi-layer protection
#[derive(Debug, Clone)]
pub struct RiskOverlay {
    // Portfolio-level limits
    max_portfolio_exposure_usd: f64,
    max_daily_loss_usd: f64,
    max_concentration_pct: f64,

    // Position-level limits
    max_position_size_usd: f64,
    _max_leverage: f64,
    max_correlation: f64,

    // Time-based limits
    max_trades_per_hour: u32,
    _max_trades_per_day: u32,
    _cooldown_period_minutes: u32,

    // Dynamic tracking
    daily_pnl: f64,
    current_positions: HashMap<String, PositionRisk>,
    trade_history: VecDeque<TradeRecord>,
    correlation_matrix: HashMap<String, HashMap<String, f64>>,
    volatility_estimates: HashMap<String, f64>,

    // Risk events
    circuit_breaker_triggered: bool,
    _last_risk_check: DateTime<Utc>,
    risk_events: VecDeque<RiskEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionRisk {
    symbol: String,
    side: Side,
    size_usd: f64,
    entry_price: f64,
    current_price: f64,
    unrealized_pnl: f64,
    value_at_risk_95: f64,
    duration_hours: f64,
    correlation_exposure: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    timestamp: DateTime<Utc>,
    symbol: String,
    side: Side,
    size_usd: f64,
    price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskEvent {
    timestamp: DateTime<Utc>,
    event_type: RiskEventType,
    severity: RiskSeverity,
    description: String,
    symbol: Option<String>,
    action_taken: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskEventType {
    PositionLimit,
    PortfolioLimit,
    CorrelationLimit,
    VolatilitySpike,
    DrawdownLimit,
    LiquidityDrop,
    ConcentrationRisk,
    TradeFrequency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub approved: bool,
    pub risk_score: f64,
    pub warnings: Vec<String>,
    pub required_adjustments: Vec<String>,
    pub max_position_size: f64,
    pub recommended_stop_loss: Option<f64>,
    pub correlation_impact: f64,
}

impl RiskOverlay {
    pub fn new() -> Self {
        Self {
            // Portfolio limits (for $200 → $1M journey)
            max_portfolio_exposure_usd: 180.0, // 90% of capital
            max_daily_loss_usd: 20.0,          // 10% daily max loss
            max_concentration_pct: 25.0,       // 25% max in any single asset

            // Position limits
            max_position_size_usd: 40.0, // $40 max position
            _max_leverage: 1.0,          // No leverage initially
            max_correlation: 0.7,        // Max correlation between positions

            // Trading frequency limits
            max_trades_per_hour: 10,
            _max_trades_per_day: 50,
            _cooldown_period_minutes: 5,

            // State tracking
            daily_pnl: 0.0,
            current_positions: HashMap::new(),
            trade_history: VecDeque::with_capacity(1000),
            correlation_matrix: HashMap::new(),
            volatility_estimates: HashMap::new(),

            circuit_breaker_triggered: false,
            _last_risk_check: Utc::now(),
            risk_events: VecDeque::with_capacity(100),
        }
    }

    /// Primary risk assessment for incoming trade signals
    pub fn assess_trade_risk(
        &mut self,
        symbol: &str,
        side: Side,
        proposed_size_usd: f64,
        current_price: f64,
        _risk_metrics: &RiskMetrics,
    ) -> Result<RiskAssessment> {
        if self.circuit_breaker_triggered {
            return Ok(RiskAssessment {
                approved: false,
                risk_score: 100.0,
                warnings: vec!["Circuit breaker is active".to_string()],
                required_adjustments: vec!["Wait for circuit breaker reset".to_string()],
                max_position_size: 0.0,
                recommended_stop_loss: None,
                correlation_impact: 0.0,
            });
        }

        let mut warnings = Vec::new();
        let mut adjustments = Vec::new();
        let mut risk_score = 0.0;

        // 1. Portfolio exposure check
        let total_exposure = self.calculate_total_exposure();
        if total_exposure + proposed_size_usd > self.max_portfolio_exposure_usd {
            warnings.push(format!(
                "Would exceed portfolio limit (${:.0} + ${:.0} > ${:.0})",
                total_exposure, proposed_size_usd, self.max_portfolio_exposure_usd
            ));
            risk_score += 25.0;
        }

        // 2. Position size check
        let current_position_size = self
            .current_positions
            .get(symbol)
            .map(|p| p.size_usd)
            .unwrap_or(0.0);

        if current_position_size + proposed_size_usd > self.max_position_size_usd {
            let max_additional = self.max_position_size_usd - current_position_size;
            adjustments.push(format!(
                "Reduce position size to ${:.0} (max additional: ${:.0})",
                max_additional, max_additional
            ));
            risk_score += 20.0;
        }

        // 3. Concentration risk check
        let portfolio_value = self.calculate_portfolio_value();
        let concentration_pct =
            (current_position_size + proposed_size_usd) / portfolio_value * 100.0;
        if concentration_pct > self.max_concentration_pct {
            warnings.push(format!(
                "High concentration risk: {:.1}% in {}",
                concentration_pct, symbol
            ));
            risk_score += 15.0;
        }

        // 4. Correlation risk check
        let correlation_impact = self.calculate_correlation_impact(symbol, proposed_size_usd);
        if correlation_impact > self.max_correlation {
            warnings.push(format!(
                "High correlation exposure: {:.2} correlation impact",
                correlation_impact
            ));
            risk_score += 10.0;
        }

        // 5. Trade frequency check
        let recent_trades = self.count_recent_trades(Duration::hours(1));
        if recent_trades >= self.max_trades_per_hour {
            warnings.push("Approaching hourly trade limit".to_string());
            risk_score += 15.0;
        }

        // 6. Volatility check
        let volatility = self.volatility_estimates.get(symbol).unwrap_or(&0.3);
        if *volatility > 0.5 {
            warnings.push(format!("High volatility asset: {:.1}%", volatility * 100.0));
            risk_score += 10.0;
        }

        // 7. Daily loss check
        if self.daily_pnl < -self.max_daily_loss_usd {
            return Ok(RiskAssessment {
                approved: false,
                risk_score: 100.0,
                warnings: vec!["Daily loss limit exceeded".to_string()],
                required_adjustments: vec!["No new positions until next day".to_string()],
                max_position_size: 0.0,
                recommended_stop_loss: None,
                correlation_impact,
            });
        }

        // Calculate adjusted position size
        let max_size = self.calculate_max_position_size(symbol, portfolio_value);
        let adjusted_size = proposed_size_usd.min(max_size);

        // Calculate recommended stop loss
        let recommended_stop_loss = self.calculate_recommended_stop_loss(
            symbol,
            side,
            current_price,
            adjusted_size,
            *volatility,
        );

        let approved = risk_score < 75.0 && adjusted_size > 5.0; // Minimum $5 position

        if !approved {
            self.record_risk_event(
                RiskEventType::PositionLimit,
                RiskSeverity::Medium,
                format!(
                    "Trade rejected for {}: risk score {:.1}",
                    symbol, risk_score
                ),
                Some(symbol.to_string()),
                "Trade rejected".to_string(),
            );
        }

        Ok(RiskAssessment {
            approved,
            risk_score,
            warnings,
            required_adjustments: adjustments,
            max_position_size: adjusted_size,
            recommended_stop_loss,
            correlation_impact,
        })
    }

    /// Update portfolio state after trade execution
    pub fn record_trade(
        &mut self,
        symbol: &str,
        side: Side,
        size_usd: f64,
        price: f64,
        timestamp: DateTime<Utc>,
    ) -> Result<()> {
        // Update position tracking
        let position = self
            .current_positions
            .entry(symbol.to_string())
            .or_insert(PositionRisk {
                symbol: symbol.to_string(),
                side,
                size_usd: 0.0,
                entry_price: price,
                current_price: price,
                unrealized_pnl: 0.0,
                value_at_risk_95: 0.0,
                duration_hours: 0.0,
                correlation_exposure: 0.0,
            });

        match side {
            Side::Long => {
                position.size_usd += size_usd;
            }
            Side::Short => {
                position.size_usd -= size_usd;
            }
        }

        // Record trade in history
        self.trade_history.push_back(TradeRecord {
            timestamp,
            symbol: symbol.to_string(),
            side,
            size_usd,
            price,
        });

        // Keep only recent history
        let cutoff = timestamp - Duration::days(1);
        while let Some(front) = self.trade_history.front() {
            if front.timestamp < cutoff {
                self.trade_history.pop_front();
            } else {
                break;
            }
        }

        debug!(
            symbol = symbol,
            side = ?side,
            size_usd = size_usd,
            price = price,
            "Trade recorded in risk overlay"
        );

        Ok(())
    }

    /// Update positions with current market prices and PnL
    pub fn update_market_data(
        &mut self,
        symbol: &str,
        current_price: f64,
        volatility: f64,
    ) -> Result<()> {
        // Update volatility estimate
        self.volatility_estimates
            .insert(symbol.to_string(), volatility);

        // Update position if exists
        if let Some(position) = self.current_positions.get_mut(symbol) {
            position.current_price = current_price;

            // Calculate unrealized PnL
            position.unrealized_pnl = match position.side {
                Side::Long => {
                    (current_price - position.entry_price)
                        * (position.size_usd / position.entry_price)
                }
                Side::Short => {
                    (position.entry_price - current_price)
                        * (position.size_usd / position.entry_price)
                }
            };

            // Calculate VaR at 95% confidence
            position.value_at_risk_95 = position.size_usd * volatility * 1.65; // Normal distribution 95% VaR
        }

        Ok(())
    }

    /// Check for circuit breaker conditions
    pub fn check_circuit_breaker(&mut self) -> Result<bool> {
        let total_unrealized_pnl: f64 = self
            .current_positions
            .values()
            .map(|p| p.unrealized_pnl)
            .sum();

        let total_pnl = self.daily_pnl + total_unrealized_pnl;

        if total_pnl < -self.max_daily_loss_usd {
            self.circuit_breaker_triggered = true;

            self.record_risk_event(
                RiskEventType::DrawdownLimit,
                RiskSeverity::Critical,
                format!("Circuit breaker triggered: ${:.2} loss", total_pnl.abs()),
                None,
                "All trading halted".to_string(),
            );

            warn!(
                daily_pnl = self.daily_pnl,
                unrealized_pnl = total_unrealized_pnl,
                total_pnl = total_pnl,
                limit = -self.max_daily_loss_usd,
                "🚨 CIRCUIT BREAKER TRIGGERED - Trading halted"
            );

            return Ok(true);
        }

        Ok(false)
    }

    /// Reset circuit breaker (typically at start of new trading day)
    pub fn reset_circuit_breaker(&mut self, new_day: bool) {
        if new_day {
            self.daily_pnl = 0.0;
        }

        self.circuit_breaker_triggered = false;

        info!("Circuit breaker reset, trading resumed");
    }

    /// Generate comprehensive risk report
    pub fn generate_risk_report(&self) -> RiskReport {
        let total_exposure = self.calculate_total_exposure();
        let portfolio_value = self.calculate_portfolio_value();
        let total_var_95 = self
            .current_positions
            .values()
            .map(|p| p.value_at_risk_95)
            .sum::<f64>();

        let exposure_ratio = total_exposure / self.max_portfolio_exposure_usd;
        let concentration_risk = self.calculate_max_concentration();
        let correlation_risk = self.calculate_average_correlation();

        RiskReport {
            timestamp: Utc::now(),
            total_exposure_usd: total_exposure,
            exposure_utilization_pct: exposure_ratio * 100.0,
            portfolio_value_usd: portfolio_value,
            daily_pnl: self.daily_pnl,
            value_at_risk_95_usd: total_var_95,
            max_concentration_pct: concentration_risk,
            average_correlation: correlation_risk,
            active_positions: self.current_positions.len(),
            circuit_breaker_active: self.circuit_breaker_triggered,
            recent_risk_events: self.risk_events.len(),
            trades_today: self.count_recent_trades(Duration::hours(24)),
        }
    }

    // Private helper methods
    fn calculate_total_exposure(&self) -> f64 {
        self.current_positions
            .values()
            .map(|p| p.size_usd.abs())
            .sum()
    }

    fn calculate_portfolio_value(&self) -> f64 {
        // Start with initial capital and add all PnL
        let total_unrealized: f64 = self
            .current_positions
            .values()
            .map(|p| p.unrealized_pnl)
            .sum();

        200.0 + self.daily_pnl + total_unrealized // Starting with $200
    }

    fn calculate_correlation_impact(&self, symbol: &str, size_usd: f64) -> f64 {
        let mut total_correlation_exposure = 0.0;

        for (other_symbol, position) in &self.current_positions {
            if other_symbol != symbol {
                if let Some(correlation_map) = self.correlation_matrix.get(symbol) {
                    if let Some(correlation) = correlation_map.get(other_symbol) {
                        total_correlation_exposure += correlation.abs() * position.size_usd;
                    }
                }
            }
        }

        total_correlation_exposure / (size_usd + self.calculate_total_exposure())
    }

    fn calculate_max_position_size(&self, symbol: &str, portfolio_value: f64) -> f64 {
        let max_by_limit = self.max_position_size_usd;
        let max_by_concentration = portfolio_value * (self.max_concentration_pct / 100.0);
        let current_position = self
            .current_positions
            .get(symbol)
            .map(|p| p.size_usd.abs())
            .unwrap_or(0.0);

        (max_by_limit.min(max_by_concentration) - current_position).max(0.0)
    }

    fn calculate_recommended_stop_loss(
        &self,
        _symbol: &str,
        side: Side,
        price: f64,
        _size_usd: f64,
        volatility: f64,
    ) -> Option<f64> {
        // Conservative stop loss: 2x daily volatility or max 5% loss
        let volatility_stop = price * (1.0 - 2.0 * volatility);
        let percentage_stop = match side {
            Side::Long => price * 0.95,  // 5% stop loss
            Side::Short => price * 1.05, // 5% stop loss
        };

        match side {
            Side::Long => Some(volatility_stop.max(percentage_stop)),
            Side::Short => Some(volatility_stop.min(percentage_stop)),
        }
    }

    fn calculate_max_concentration(&self) -> f64 {
        if self.current_positions.is_empty() {
            return 0.0;
        }

        let portfolio_value = self.calculate_portfolio_value();
        self.current_positions
            .values()
            .map(|p| p.size_usd.abs() / portfolio_value * 100.0)
            .fold(0.0, f64::max)
    }

    fn calculate_average_correlation(&self) -> f64 {
        if self.current_positions.len() < 2 {
            return 0.0;
        }

        let mut total_correlation = 0.0;
        let mut count = 0;

        for symbol1 in self.current_positions.keys() {
            for symbol2 in self.current_positions.keys() {
                if symbol1 != symbol2 {
                    if let Some(correlation_map) = self.correlation_matrix.get(symbol1) {
                        if let Some(correlation) = correlation_map.get(symbol2) {
                            total_correlation += correlation.abs();
                            count += 1;
                        }
                    }
                }
            }
        }

        if count > 0 {
            total_correlation / count as f64
        } else {
            0.0
        }
    }

    fn count_recent_trades(&self, period: Duration) -> u32 {
        let cutoff = Utc::now() - period;
        self.trade_history
            .iter()
            .filter(|trade| trade.timestamp >= cutoff)
            .count() as u32
    }

    fn record_risk_event(
        &mut self,
        event_type: RiskEventType,
        severity: RiskSeverity,
        description: String,
        symbol: Option<String>,
        action_taken: String,
    ) {
        let event = RiskEvent {
            timestamp: Utc::now(),
            event_type,
            severity,
            description,
            symbol,
            action_taken,
        };

        self.risk_events.push_back(event);

        // Keep only recent events
        if self.risk_events.len() > 100 {
            self.risk_events.pop_front();
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskReport {
    pub timestamp: DateTime<Utc>,
    pub total_exposure_usd: f64,
    pub exposure_utilization_pct: f64,
    pub portfolio_value_usd: f64,
    pub daily_pnl: f64,
    pub value_at_risk_95_usd: f64,
    pub max_concentration_pct: f64,
    pub average_correlation: f64,
    pub active_positions: usize,
    pub circuit_breaker_active: bool,
    pub recent_risk_events: usize,
    pub trades_today: u32,
}

impl Default for RiskOverlay {
    fn default() -> Self {
        Self::new()
    }
}
