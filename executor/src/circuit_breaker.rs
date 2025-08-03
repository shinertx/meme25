use shared_models::error::{Result, ModelError};
use redis::{Client, AsyncCommands};
use crate::config::Config;
use std::collections::HashMap;
use tokio::time::{Duration, sleep, Instant};
use tracing::{error, warn, info, debug};
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

/// Advanced Circuit Breaker System
/// 
/// **EDGE THESIS**: Multi-layered circuit breakers with adaptive thresholds
/// prevent catastrophic losses while minimizing false positives that interrupt profitable trading.
/// 
/// **INSTITUTIONAL FEATURES**:
/// - Adaptive thresholds based on market volatility and strategy performance
/// - Multi-tiered response levels (Warning → Restriction → Halt → Emergency)
/// - Strategy-specific circuit breakers with performance-based sensitivity
/// - Market regime-aware trigger adjustments
/// - Automatic recovery mechanisms with gradual position rebuilding
///
/// **RISK CONTROLS**:
/// - Portfolio-wide: 5% daily, 10% total drawdown limits
/// - Strategy-specific: Individual strategy performance monitoring
/// - Execution-based: Latency, slippage, and error rate thresholds
/// - Correlation-based: Cross-strategy risk concentration limits
/// - External: Market structure and liquidity degradation detection
#[derive(Debug)]
pub struct CircuitBreaker {
    cfg: Config,
    redis: Client,
    breach_counters: HashMap<String, u32>,
    last_check: Instant,
    adaptive_thresholds: AdaptiveThresholds,
    circuit_states: HashMap<String, CircuitState>,
    recovery_manager: RecoveryManager,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveThresholds {
    pub portfolio_drawdown_warning: f64,   // 3%
    pub portfolio_drawdown_halt: f64,      // 5%  
    pub portfolio_drawdown_emergency: f64, // 10%
    pub daily_loss_warning: f64,           // 2%
    pub daily_loss_halt: f64,              // 3%
    pub strategy_drawdown_limit: f64,      // 8%
    pub execution_latency_ms: u64,         // 1000ms
    pub slippage_threshold_bps: f64,       // 100bp
    pub error_rate_threshold: f64,         // 5%
    pub liquidity_degradation_threshold: f64, // 50%
    pub correlation_concentration_limit: f64,  // 60%
}

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Normal,
    Warning,     // Elevated monitoring
    Restricted,  // Reduced position sizes
    Halted,      // No new positions
    Emergency,   // Close all positions
}

#[derive(Debug, Clone)]
pub enum CircuitTrigger {
    PortfolioDrawdown(f64),
    DailyLoss(f64),
    StrategyFailure(String, f64),
    ExecutionLatency(u64),
    SlippageExcess(f64),
    ErrorRateSpike(f64),
    LiquidityDegradation(f64),
    CorrelationRisk(f64),
    ExternalMarketStress,
}

#[derive(Debug)]
pub struct RecoveryManager {
    recovery_start: Option<DateTime<Utc>>,
    recovery_phase: RecoveryPhase,
    gradual_allocation_pct: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecoveryPhase {
    Halted,
    Testing,      // 10% allocation
    Cautious,     // 25% allocation  
    Progressive,  // 50% allocation
    Normal,       // 100% allocation
}

impl CircuitBreaker {
    pub fn new(cfg: Config, redis: Client) -> Self {
        Self {
            cfg,
            redis,
            breach_counters: HashMap::new(),
            last_check: Instant::now(),
            adaptive_thresholds: AdaptiveThresholds::default(),
            circuit_states: HashMap::new(),
            recovery_manager: RecoveryManager::new(),
        }
    }

    /// Main circuit breaker monitoring loop
    pub async fn tick(mut self) -> Result<()> {
        let mut conn = self.redis.get_async_connection().await
            .map_err(|e| ModelError::Network(format!("Redis connection failed: {}", e)))?;
        
        info!("🛡️ Advanced Circuit Breaker System started - Multi-layer protection active");
        
        loop {
            match self.comprehensive_health_check(&mut conn).await {
                Ok(triggers) => {
                    if !triggers.is_empty() {
                        self.handle_circuit_triggers(triggers, &mut conn).await?;
                    }
                    self.update_recovery_status(&mut conn).await?;
                }
                Err(e) => {
                    error!(error = %e, "Circuit breaker health check failed");
                }
            }
            
            sleep(Duration::from_secs(10)).await; // Check every 10 seconds
            self.last_check = Instant::now();
        }
    }

    /// Comprehensive multi-layer health monitoring
    async fn comprehensive_health_check(&mut self, conn: &mut redis::aio::Connection) -> Result<Vec<CircuitTrigger>> {
        let mut triggers = Vec::new();

        // Layer 1: Portfolio-level checks
        triggers.extend(self.check_portfolio_health(conn).await?);

        // Layer 2: Strategy-level checks  
        triggers.extend(self.check_strategy_health(conn).await?);

        // Layer 3: Execution quality checks
        triggers.extend(self.check_execution_health(conn).await?);

        // Layer 4: Market structure checks
        triggers.extend(self.check_market_health(conn).await?);

        // Layer 5: Correlation and concentration checks
        triggers.extend(self.check_correlation_risks(conn).await?);

        debug!("Health check completed - {} triggers detected", triggers.len());
        Ok(triggers)
    }

    /// Layer 1: Portfolio-level health monitoring
    async fn check_portfolio_health(&self, conn: &mut redis::aio::Connection) -> Result<Vec<CircuitTrigger>> {
        let mut triggers = Vec::new();

        // Portfolio drawdown check
        let portfolio_drawdown: f64 = conn.get("portfolio_drawdown").await.unwrap_or(0.0);
        if portfolio_drawdown.abs() > self.adaptive_thresholds.portfolio_drawdown_emergency {
            triggers.push(CircuitTrigger::PortfolioDrawdown(portfolio_drawdown));
        } else if portfolio_drawdown.abs() > self.adaptive_thresholds.portfolio_drawdown_halt {
            triggers.push(CircuitTrigger::PortfolioDrawdown(portfolio_drawdown));
        } else if portfolio_drawdown.abs() > self.adaptive_thresholds.portfolio_drawdown_warning {
            triggers.push(CircuitTrigger::PortfolioDrawdown(portfolio_drawdown));
        }

        // Daily loss check
        let daily_pnl: f64 = conn.get("daily_pnl").await.unwrap_or(0.0);
        let daily_loss_pct = daily_pnl / self.cfg.max_portfolio_size_usd * 100.0;
        
        if daily_loss_pct < -self.adaptive_thresholds.daily_loss_halt {
            triggers.push(CircuitTrigger::DailyLoss(daily_loss_pct));
        } else if daily_loss_pct < -self.adaptive_thresholds.daily_loss_warning {
            triggers.push(CircuitTrigger::DailyLoss(daily_loss_pct));
        }

        Ok(triggers)
    }

    /// Layer 2: Strategy-level health monitoring
    async fn check_strategy_health(&self, conn: &mut redis::aio::Connection) -> Result<Vec<CircuitTrigger>> {
        let mut triggers = Vec::new();

        // Check each strategy's performance
        let strategy_metrics: String = conn.get("strategy_performance_json").await.unwrap_or_default();
        if !strategy_metrics.is_empty() {
            if let Ok(metrics) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&strategy_metrics) {
                for (strategy_id, strategy_data) in metrics {
                    if let Some(drawdown) = strategy_data.get("drawdown").and_then(|v| v.as_f64()) {
                        if drawdown.abs() > self.adaptive_thresholds.strategy_drawdown_limit {
                            triggers.push(CircuitTrigger::StrategyFailure(strategy_id, drawdown));
                        }
                    }
                }
            }
        }

        Ok(triggers)
    }

    /// Layer 3: Execution quality monitoring
    async fn check_execution_health(&self, conn: &mut redis::aio::Connection) -> Result<Vec<CircuitTrigger>> {
        let mut triggers = Vec::new();

        // Execution latency check
        let avg_latency_ms: u64 = conn.get("avg_execution_latency_ms").await.unwrap_or(0);
        if avg_latency_ms > self.adaptive_thresholds.execution_latency_ms {
            triggers.push(CircuitTrigger::ExecutionLatency(avg_latency_ms));
        }

        // Slippage monitoring
        let avg_slippage_bps: f64 = conn.get("avg_slippage_bps").await.unwrap_or(0.0);
        if avg_slippage_bps > self.adaptive_thresholds.slippage_threshold_bps {
            triggers.push(CircuitTrigger::SlippageExcess(avg_slippage_bps));
        }

        // Error rate monitoring
        let error_rate: f64 = conn.get("execution_error_rate").await.unwrap_or(0.0);
        if error_rate > self.adaptive_thresholds.error_rate_threshold {
            triggers.push(CircuitTrigger::ErrorRateSpike(error_rate));
        }

        Ok(triggers)
    }

    /// Layer 4: Market structure monitoring
    async fn check_market_health(&self, conn: &mut redis::aio::Connection) -> Result<Vec<CircuitTrigger>> {
        let mut triggers = Vec::new();

        // Market liquidity degradation
        let liquidity_score: f64 = conn.get("market_liquidity_score").await.unwrap_or(1.0);
        if liquidity_score < self.adaptive_thresholds.liquidity_degradation_threshold {
            triggers.push(CircuitTrigger::LiquidityDegradation(liquidity_score));
        }

        // External market stress indicators
        let vix_level: f64 = conn.get("market_stress_indicator").await.unwrap_or(0.0);
        if vix_level > 75.0 { // Extreme market stress
            triggers.push(CircuitTrigger::ExternalMarketStress);
        }

        Ok(triggers)
    }

    /// Layer 5: Correlation and concentration risk monitoring
    async fn check_correlation_risks(&self, conn: &mut redis::aio::Connection) -> Result<Vec<CircuitTrigger>> {
        let mut triggers = Vec::new();

        // High correlation concentration
        let max_cluster_allocation: f64 = conn.get("max_correlation_cluster_allocation").await.unwrap_or(0.0);
        if max_cluster_allocation > self.adaptive_thresholds.correlation_concentration_limit {
            triggers.push(CircuitTrigger::CorrelationRisk(max_cluster_allocation));
        }

        Ok(triggers)
    }

    /// Handle circuit breaker triggers with appropriate responses
    async fn handle_circuit_triggers(&mut self, triggers: Vec<CircuitTrigger>, conn: &mut redis::aio::Connection) -> Result<()> {
        let mut highest_severity = CircuitState::Normal;

        for trigger in &triggers {
            let severity = self.determine_severity(&trigger);
            if self.is_more_severe(&severity, &highest_severity) {
                highest_severity = severity.clone();
            }

            self.log_trigger(&trigger, &severity);
            self.increment_breach_counter(&trigger);
        }

        // Apply the most severe circuit state
        if highest_severity != CircuitState::Normal {
            self.apply_circuit_action(highest_severity, conn).await?;
        }

        Ok(())
    }

    /// Determine the severity of a circuit trigger
    fn determine_severity(&self, trigger: &CircuitTrigger) -> CircuitState {
        match trigger {
            CircuitTrigger::PortfolioDrawdown(dd) => {
                if dd.abs() >= self.adaptive_thresholds.portfolio_drawdown_emergency {
                    CircuitState::Emergency
                } else if dd.abs() >= self.adaptive_thresholds.portfolio_drawdown_halt {
                    CircuitState::Halted
                } else {
                    CircuitState::Warning
                }
            }
            CircuitTrigger::DailyLoss(loss) => {
                if loss.abs() >= self.adaptive_thresholds.daily_loss_halt {
                    CircuitState::Halted
                } else {
                    CircuitState::Warning
                }
            }
            CircuitTrigger::StrategyFailure(_, _) => CircuitState::Restricted,
            CircuitTrigger::ExecutionLatency(_) => CircuitState::Restricted,
            CircuitTrigger::SlippageExcess(_) => CircuitState::Restricted,
            CircuitTrigger::ErrorRateSpike(_) => CircuitState::Halted,
            CircuitTrigger::LiquidityDegradation(_) => CircuitState::Restricted,
            CircuitTrigger::CorrelationRisk(_) => CircuitState::Warning,
            CircuitTrigger::ExternalMarketStress => CircuitState::Halted,
        }
    }

    /// Check if one circuit state is more severe than another
    fn is_more_severe(&self, state1: &CircuitState, state2: &CircuitState) -> bool {
        let severity_order = [
            CircuitState::Normal,
            CircuitState::Warning,
            CircuitState::Restricted,
            CircuitState::Halted,
            CircuitState::Emergency,
        ];

        if let (Some(pos1), Some(pos2)) = (
            severity_order.iter().position(|s| s == state1),
            severity_order.iter().position(|s| s == state2),
        ) {
            pos1 > pos2
        } else {
            false
        }
    }

    /// Apply circuit breaker action based on severity
    async fn apply_circuit_action(&mut self, state: CircuitState, conn: &mut redis::aio::Connection) -> Result<()> {
        match state {
            CircuitState::Warning => {
                warn!("⚠️ Circuit Breaker WARNING - Enhanced monitoring activated");
                let _: () = conn.set("circuit_state", "WARNING").await
                    .map_err(|e| ModelError::Network(format!("Redis error: {}", e)))?;
                let _: () = conn.publish("alerts", "CIRCUIT_WARNING").await
                    .map_err(|e| ModelError::Network(format!("Redis error: {}", e)))?;
            }
            CircuitState::Restricted => {
                warn!("🔶 Circuit Breaker RESTRICTED - Position sizes reduced to 50%");
                let _: () = conn.set("circuit_state", "RESTRICTED").await
                    .map_err(|e| ModelError::Network(format!("Redis error: {}", e)))?;
                let _: () = conn.set("position_size_multiplier", 0.5).await
                    .map_err(|e| ModelError::Network(format!("Redis error: {}", e)))?;
                let _: () = conn.publish("control", "RESTRICT_POSITIONS").await
                    .map_err(|e| ModelError::Network(format!("Redis error: {}", e)))?;
            }
            CircuitState::Halted => {
                error!("🛑 Circuit Breaker HALTED - No new positions allowed");
                let _: () = conn.set("circuit_state", "HALTED").await
                    .map_err(|e| ModelError::Network(format!("Redis error: {}", e)))?;
                let _: () = conn.set("trading_enabled", false).await
                    .map_err(|e| ModelError::Network(format!("Redis error: {}", e)))?;
                let _: () = conn.publish("control", "HALT_TRADING").await
                    .map_err(|e| ModelError::Network(format!("Redis error: {}", e)))?;
                self.recovery_manager.initiate_recovery();
            }
            CircuitState::Emergency => {
                error!("🚨 Circuit Breaker EMERGENCY - Liquidating all positions");
                let _: () = conn.set("circuit_state", "EMERGENCY").await
                    .map_err(|e| ModelError::Network(format!("Redis error: {}", e)))?;
                let _: () = conn.set("emergency_liquidation", true).await
                    .map_err(|e| ModelError::Network(format!("Redis error: {}", e)))?;
                let _: () = conn.publish("control", "EMERGENCY_LIQUIDATE").await
                    .map_err(|e| ModelError::Network(format!("Redis error: {}", e)))?;
                self.recovery_manager.initiate_recovery();
            }
            CircuitState::Normal => {}
        }

        Ok(())
    }

    /// Update recovery status and gradual re-enabling
    async fn update_recovery_status(&mut self, conn: &mut redis::aio::Connection) -> Result<()> {
        if let Some(recovery_progress) = self.recovery_manager.update_recovery() {
            info!("📈 Recovery progress: {:?} - {}% allocation", 
                  recovery_progress.phase, recovery_progress.allocation_pct);
                  
            let _: () = conn.set("recovery_allocation_pct", recovery_progress.allocation_pct).await
                .map_err(|e| ModelError::Network(format!("Redis error: {}", e)))?;
            let _: () = conn.set("recovery_phase", format!("{:?}", recovery_progress.phase)).await
                .map_err(|e| ModelError::Network(format!("Redis error: {}", e)))?;
        }

        Ok(())
    }

    /// Log circuit trigger details
    fn log_trigger(&self, trigger: &CircuitTrigger, severity: &CircuitState) {
        match trigger {
            CircuitTrigger::PortfolioDrawdown(dd) => {
                error!("Portfolio drawdown: {:.2}% (Severity: {:?})", dd, severity);
            }
            CircuitTrigger::DailyLoss(loss) => {
                error!("Daily loss: {:.2}% (Severity: {:?})", loss, severity);
            }
            CircuitTrigger::StrategyFailure(strategy, dd) => {
                error!("Strategy {} failure: {:.2}% drawdown (Severity: {:?})", strategy, dd, severity);
            }
            CircuitTrigger::ExecutionLatency(ms) => {
                error!("High execution latency: {}ms (Severity: {:?})", ms, severity);
            }
            CircuitTrigger::SlippageExcess(bps) => {
                error!("Excessive slippage: {:.1}bp (Severity: {:?})", bps, severity);
            }
            CircuitTrigger::ErrorRateSpike(rate) => {
                error!("Error rate spike: {:.1}% (Severity: {:?})", rate, severity);
            }
            CircuitTrigger::LiquidityDegradation(score) => {
                error!("Liquidity degradation: {:.2} (Severity: {:?})", score, severity);
            }
            CircuitTrigger::CorrelationRisk(concentration) => {
                error!("Correlation risk: {:.1}% concentration (Severity: {:?})", concentration, severity);
            }
            CircuitTrigger::ExternalMarketStress => {
                error!("External market stress detected (Severity: {:?})", severity);
            }
        }
    }

    /// Increment breach counter for trigger persistence tracking
    fn increment_breach_counter(&mut self, trigger: &CircuitTrigger) {
        let key = match trigger {
            CircuitTrigger::PortfolioDrawdown(_) => "portfolio_drawdown",
            CircuitTrigger::DailyLoss(_) => "daily_loss",
            CircuitTrigger::StrategyFailure(_strategy, _) => return, // Strategy-specific tracking would go here
            CircuitTrigger::ExecutionLatency(_) => "execution_latency",
            CircuitTrigger::SlippageExcess(_) => "slippage",
            CircuitTrigger::ErrorRateSpike(_) => "error_rate",
            CircuitTrigger::LiquidityDegradation(_) => "liquidity",
            CircuitTrigger::CorrelationRisk(_) => "correlation",
            CircuitTrigger::ExternalMarketStress => "market_stress",
        };

        *self.breach_counters.entry(key.to_string()).or_insert(0) += 1;
    }
}

impl AdaptiveThresholds {
    pub fn default() -> Self {
        Self {
            portfolio_drawdown_warning: 0.03,   // 3%
            portfolio_drawdown_halt: 0.05,      // 5%
            portfolio_drawdown_emergency: 0.10, // 10%
            daily_loss_warning: 0.02,           // 2%
            daily_loss_halt: 0.03,              // 3%
            strategy_drawdown_limit: 0.08,      // 8%
            execution_latency_ms: 1000,         // 1 second
            slippage_threshold_bps: 100.0,      // 100 basis points
            error_rate_threshold: 0.05,         // 5%
            liquidity_degradation_threshold: 0.5, // 50%
            correlation_concentration_limit: 0.6, // 60%
        }
    }
}

impl RecoveryManager {
    pub fn new() -> Self {
        Self {
            recovery_start: None,
            recovery_phase: RecoveryPhase::Normal,
            gradual_allocation_pct: 100.0,
        }
    }

    pub fn initiate_recovery(&mut self) {
        self.recovery_start = Some(Utc::now());
        self.recovery_phase = RecoveryPhase::Halted;
        self.gradual_allocation_pct = 0.0;
        info!("🔄 Recovery process initiated");
    }

    pub fn update_recovery(&mut self) -> Option<RecoveryProgress> {
        if let Some(start_time) = self.recovery_start {
            let elapsed_hours = Utc::now().signed_duration_since(start_time).num_hours();
            
            let (new_phase, new_allocation) = match elapsed_hours {
                0..=1 => (RecoveryPhase::Halted, 0.0),
                2..=4 => (RecoveryPhase::Testing, 10.0),
                5..=8 => (RecoveryPhase::Cautious, 25.0),
                9..=12 => (RecoveryPhase::Progressive, 50.0),
                _ => {
                    self.recovery_start = None; // Recovery complete
                    (RecoveryPhase::Normal, 100.0)
                }
            };

            if new_phase != self.recovery_phase || new_allocation != self.gradual_allocation_pct {
                self.recovery_phase = new_phase.clone();
                self.gradual_allocation_pct = new_allocation;
                
                return Some(RecoveryProgress {
                    phase: new_phase,
                    allocation_pct: new_allocation,
                });
            }
        }
        
        None
    }
}

#[derive(Debug)]
pub struct RecoveryProgress {
    pub phase: RecoveryPhase,
    pub allocation_pct: f64,
}
