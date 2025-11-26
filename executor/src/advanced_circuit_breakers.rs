use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use shared_models::error::Result;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BreakerType {
    Portfolio,   // System-wide portfolio protection
    Strategy,    // Per-strategy limits
    Position,    // Individual position limits
    Market,      // Market-wide conditions
    Volatility,  // Volatility-based limits
    Correlation, // Cross-asset correlation limits
    Liquidity,   // Liquidity-based protection
    Volume,      // Volume-based limits
    Drawdown,    // Maximum drawdown protection
    VaR,         // Value at Risk limits
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BreakerSeverity {
    Warning,   // Soft limit - log warning
    Throttle,  // Reduce position sizes
    Pause,     // Pause strategy/system temporarily
    Stop,      // Hard stop - cease all activity
    Emergency, // Emergency stop with immediate liquidation
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BreakerState {
    Armed,     // Normal operation, monitoring
    Triggered, // Breaker has been triggered
    Recovery,  // In recovery mode, gradually resuming
    Disabled,  // Manually disabled
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakerConfig {
    pub breaker_type: BreakerType,
    pub name: String,
    pub description: String,
    pub threshold: f64,
    pub severity: BreakerSeverity,
    pub lookback_period_minutes: u32,
    pub recovery_time_minutes: u32,
    pub max_triggers_per_hour: u32,
    pub enabled: bool,
    pub auto_recovery: bool,
    pub cascading_impact: Vec<String>, // Other breakers to trigger
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakerTrigger {
    pub timestamp: DateTime<Utc>,
    pub breaker_name: String,
    pub trigger_value: f64,
    pub threshold: f64,
    pub severity: BreakerSeverity,
    pub message: String,
    pub affected_strategies: Vec<String>,
    pub recovery_eta: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakerMetrics {
    pub current_value: f64,
    pub threshold: f64,
    pub utilization_pct: f64,
    pub triggers_today: u32,
    pub last_trigger: Option<DateTime<Utc>>,
    pub time_since_last_trigger_minutes: Option<i64>,
    pub state: BreakerState,
}

#[derive(Debug, Clone)]
struct BreakerInstance {
    config: BreakerConfig,
    state: BreakerState,
    current_value: f64,
    trigger_history: VecDeque<BreakerTrigger>,
    last_trigger_time: Option<DateTime<Utc>>,
    recovery_start_time: Option<DateTime<Utc>>,
    trigger_count_hour: u32,
    trigger_count_day: u32,
    hour_reset_time: DateTime<Utc>,
    day_reset_time: DateTime<Utc>,
}

pub struct AdvancedCircuitBreakers {
    breakers: Arc<RwLock<HashMap<String, BreakerInstance>>>,
    global_emergency_stop: Arc<RwLock<bool>>,

    // System state tracking
    portfolio_value: f64,
    portfolio_drawdown: f64,
    market_volatility: f64,
    system_correlation: f64,

    // Activity monitoring
    recent_triggers: VecDeque<BreakerTrigger>,
    max_trigger_history: usize,

    // Configuration
    emergency_liquidation_threshold: f64,
    cascade_delay_seconds: u64,
}

impl Default for AdvancedCircuitBreakers {
    fn default() -> Self {
        Self::new()
    }
}

impl AdvancedCircuitBreakers {
    pub fn new() -> Self {
        Self {
            breakers: Arc::new(RwLock::new(HashMap::new())),
            global_emergency_stop: Arc::new(RwLock::new(false)),
            portfolio_value: 200.0, // Starting value
            portfolio_drawdown: 0.0,
            market_volatility: 0.0,
            system_correlation: 0.0,
            recent_triggers: VecDeque::new(),
            max_trigger_history: 1000,
            emergency_liquidation_threshold: 50.0, // 50% portfolio loss
            cascade_delay_seconds: 5,
        }
    }

    pub async fn initialize_default_breakers(&mut self) -> Result<()> {
        let configs = vec![
            // Portfolio Protection
            BreakerConfig {
                breaker_type: BreakerType::Portfolio,
                name: "portfolio_loss_10pct".to_string(),
                description: "Portfolio loss exceeds 10%".to_string(),
                threshold: -10.0,
                severity: BreakerSeverity::Warning,
                lookback_period_minutes: 60,
                recovery_time_minutes: 30,
                max_triggers_per_hour: 3,
                enabled: true,
                auto_recovery: true,
                cascading_impact: vec![],
            },
            BreakerConfig {
                breaker_type: BreakerType::Portfolio,
                name: "portfolio_loss_25pct".to_string(),
                description: "Portfolio loss exceeds 25% - CRITICAL".to_string(),
                threshold: -25.0,
                severity: BreakerSeverity::Stop,
                lookback_period_minutes: 1440, // 24 hours
                recovery_time_minutes: 120,
                max_triggers_per_hour: 1,
                enabled: true,
                auto_recovery: false,
                cascading_impact: vec!["all_strategies_pause".to_string()],
            },
            BreakerConfig {
                breaker_type: BreakerType::Portfolio,
                name: "portfolio_loss_40pct".to_string(),
                description: "EMERGENCY: Portfolio loss exceeds 40%".to_string(),
                threshold: -40.0,
                severity: BreakerSeverity::Emergency,
                lookback_period_minutes: 10080, // 7 days
                recovery_time_minutes: 1440,    // 24 hours
                max_triggers_per_hour: 1,
                enabled: true,
                auto_recovery: false,
                cascading_impact: vec!["emergency_liquidation".to_string()],
            },
            // Drawdown Protection
            BreakerConfig {
                breaker_type: BreakerType::Drawdown,
                name: "max_drawdown_15pct".to_string(),
                description: "Maximum drawdown exceeds 15%".to_string(),
                threshold: 15.0,
                severity: BreakerSeverity::Throttle,
                lookback_period_minutes: 720, // 12 hours
                recovery_time_minutes: 60,
                max_triggers_per_hour: 2,
                enabled: true,
                auto_recovery: true,
                cascading_impact: vec!["reduce_position_sizes".to_string()],
            },
            // Volatility Protection
            BreakerConfig {
                breaker_type: BreakerType::Volatility,
                name: "high_volatility_500pct".to_string(),
                description: "Market volatility exceeds 500%".to_string(),
                threshold: 5.0, // 500% annualized
                severity: BreakerSeverity::Pause,
                lookback_period_minutes: 60,
                recovery_time_minutes: 15,
                max_triggers_per_hour: 5,
                enabled: true,
                auto_recovery: true,
                cascading_impact: vec![],
            },
            // VaR Protection
            BreakerConfig {
                breaker_type: BreakerType::VaR,
                name: "var_95_exceeded".to_string(),
                description: "95% VaR limit exceeded".to_string(),
                threshold: 50.0, // $50 daily VaR
                severity: BreakerSeverity::Warning,
                lookback_period_minutes: 1440,
                recovery_time_minutes: 30,
                max_triggers_per_hour: 3,
                enabled: true,
                auto_recovery: true,
                cascading_impact: vec![],
            },
            // Correlation Protection
            BreakerConfig {
                breaker_type: BreakerType::Correlation,
                name: "high_correlation_90pct".to_string(),
                description: "Portfolio correlation exceeds 90%".to_string(),
                threshold: 0.9,
                severity: BreakerSeverity::Throttle,
                lookback_period_minutes: 240, // 4 hours
                recovery_time_minutes: 60,
                max_triggers_per_hour: 2,
                enabled: true,
                auto_recovery: true,
                cascading_impact: vec!["diversification_required".to_string()],
            },
            // Strategy-Level Protection
            BreakerConfig {
                breaker_type: BreakerType::Strategy,
                name: "strategy_loss_30pct".to_string(),
                description: "Individual strategy loss exceeds 30%".to_string(),
                threshold: -30.0,
                severity: BreakerSeverity::Stop,
                lookback_period_minutes: 1440,
                recovery_time_minutes: 240, // 4 hours
                max_triggers_per_hour: 1,
                enabled: true,
                auto_recovery: false,
                cascading_impact: vec![],
            },
        ];

        for config in configs {
            self.add_breaker(config).await?;
        }

        info!(
            "Initialized {} advanced circuit breakers",
            self.breakers.read().await.len()
        );
        Ok(())
    }

    pub async fn add_breaker(&mut self, config: BreakerConfig) -> Result<()> {
        let name = config.name.clone();
        let instance = BreakerInstance {
            config,
            state: BreakerState::Armed,
            current_value: 0.0,
            trigger_history: VecDeque::new(),
            last_trigger_time: None,
            recovery_start_time: None,
            trigger_count_hour: 0,
            trigger_count_day: 0,
            hour_reset_time: Utc::now() + Duration::hours(1),
            day_reset_time: Utc::now() + Duration::days(1),
        };

        self.breakers.write().await.insert(name.clone(), instance);
        debug!("Added circuit breaker: {}", name);
        Ok(())
    }

    pub async fn update_portfolio_metrics(
        &mut self,
        current_value: f64,
        drawdown_pct: f64,
        _daily_pnl: f64,
    ) -> Result<Vec<BreakerTrigger>> {
        self.portfolio_value = current_value;
        self.portfolio_drawdown = drawdown_pct;

        let mut triggers = Vec::new();

        // Calculate portfolio loss percentage
        let portfolio_loss_pct = ((200.0 - current_value) / 200.0) * 100.0;

        // Check portfolio loss breakers
        triggers.extend(
            self.check_breaker("portfolio_loss_10pct", portfolio_loss_pct)
                .await?,
        );
        triggers.extend(
            self.check_breaker("portfolio_loss_25pct", portfolio_loss_pct)
                .await?,
        );
        triggers.extend(
            self.check_breaker("portfolio_loss_40pct", portfolio_loss_pct)
                .await?,
        );

        // Check drawdown breakers
        triggers.extend(
            self.check_breaker("max_drawdown_15pct", drawdown_pct)
                .await?,
        );

        if portfolio_loss_pct >= self.emergency_liquidation_threshold {
            let emergency_trigger = BreakerTrigger {
                timestamp: Utc::now(),
                breaker_name: "emergency_liquidation".to_string(),
                trigger_value: portfolio_loss_pct,
                threshold: self.emergency_liquidation_threshold,
                severity: BreakerSeverity::Emergency,
                message: format!(
                    "Emergency liquidation threshold breached: {:.2}% loss",
                    portfolio_loss_pct
                ),
                affected_strategies: Vec::new(),
                recovery_eta: None,
            };

            *self.global_emergency_stop.write().await = true;
            self.recent_triggers.push_back(emergency_trigger.clone());
            if self.recent_triggers.len() > self.max_trigger_history {
                self.recent_triggers.pop_front();
            }

            triggers.push(emergency_trigger);
        }

        Ok(triggers)
    }

    pub async fn update_risk_metrics(
        &mut self,
        var_95: f64,
        volatility: f64,
        correlation: f64,
    ) -> Result<Vec<BreakerTrigger>> {
        self.market_volatility = volatility;
        self.system_correlation = correlation;

        let mut triggers = Vec::new();

        // Check VaR breakers
        triggers.extend(self.check_breaker("var_95_exceeded", var_95).await?);

        // Check volatility breakers (convert to percentage)
        triggers.extend(
            self.check_breaker("high_volatility_500pct", volatility * 100.0)
                .await?,
        );

        // Check correlation breakers
        triggers.extend(
            self.check_breaker("high_correlation_90pct", correlation)
                .await?,
        );

        Ok(triggers)
    }

    pub async fn update_strategy_metrics(
        &mut self,
        strategy_id: &str,
        pnl_pct: f64,
    ) -> Result<Vec<BreakerTrigger>> {
        // Check strategy-specific breakers
        let triggers = self.check_breaker("strategy_loss_30pct", pnl_pct).await?;

        // Add strategy context to triggers
        let mut contextualized_triggers = Vec::new();
        for mut trigger in triggers {
            trigger.affected_strategies.push(strategy_id.to_string());
            contextualized_triggers.push(trigger);
        }

        Ok(contextualized_triggers)
    }

    async fn check_breaker(
        &mut self,
        breaker_name: &str,
        current_value: f64,
    ) -> Result<Vec<BreakerTrigger>> {
        let mut breakers = self.breakers.write().await;
        let mut triggers = Vec::new();

        if let Some(breaker) = breakers.get_mut(breaker_name) {
            if !breaker.config.enabled || breaker.state == BreakerState::Disabled {
                return Ok(triggers);
            }

            breaker.current_value = current_value;

            // Update time-based counters
            let now = Utc::now();
            if now > breaker.hour_reset_time {
                breaker.trigger_count_hour = 0;
                breaker.hour_reset_time = now + Duration::hours(1);
            }
            if now > breaker.day_reset_time {
                breaker.trigger_count_day = 0;
                breaker.day_reset_time = now + Duration::days(1);
            }

            // Check if breaker should trigger
            let should_trigger = match breaker.config.breaker_type {
                BreakerType::Portfolio | BreakerType::Strategy | BreakerType::Drawdown => {
                    current_value >= breaker.config.threshold.abs()
                }
                BreakerType::VaR | BreakerType::Volatility | BreakerType::Correlation => {
                    current_value >= breaker.config.threshold
                }
                _ => false,
            };

            // Check recovery conditions
            if breaker.state == BreakerState::Recovery {
                if let Some(recovery_start) = breaker.recovery_start_time {
                    let recovery_duration = now.signed_duration_since(recovery_start);
                    if recovery_duration.num_minutes()
                        >= breaker.config.recovery_time_minutes as i64
                    {
                        breaker.state = BreakerState::Armed;
                        breaker.recovery_start_time = None;
                        info!("Circuit breaker '{}' recovered and re-armed", breaker_name);
                    }
                }
                return Ok(triggers); // Don't trigger while in recovery
            }

            if should_trigger && breaker.state == BreakerState::Armed {
                // Check trigger rate limits
                if breaker.trigger_count_hour >= breaker.config.max_triggers_per_hour {
                    warn!(
                        "Circuit breaker '{}' rate limited (max {} per hour)",
                        breaker_name, breaker.config.max_triggers_per_hour
                    );
                    return Ok(triggers);
                }

                // Create trigger
                let trigger = BreakerTrigger {
                    timestamp: now,
                    breaker_name: breaker_name.to_string(),
                    trigger_value: current_value,
                    threshold: breaker.config.threshold,
                    severity: breaker.config.severity.clone(),
                    message: format!(
                        "{}: {:.2} exceeds threshold {:.2}",
                        breaker.config.description, current_value, breaker.config.threshold
                    ),
                    affected_strategies: vec![],
                    recovery_eta: if breaker.config.auto_recovery {
                        Some(now + Duration::minutes(breaker.config.recovery_time_minutes as i64))
                    } else {
                        None
                    },
                };

                // Update breaker state
                breaker.state = BreakerState::Triggered;
                breaker.last_trigger_time = Some(now);
                breaker.trigger_count_hour += 1;
                breaker.trigger_count_day += 1;
                breaker.trigger_history.push_back(trigger.clone());

                // Limit trigger history
                while breaker.trigger_history.len() > 100 {
                    breaker.trigger_history.pop_front();
                }

                // Handle severity-specific actions
                match trigger.severity {
                    BreakerSeverity::Emergency => {
                        *self.global_emergency_stop.write().await = true;
                        error!("EMERGENCY STOP: {}", trigger.message);
                    }
                    BreakerSeverity::Stop => {
                        warn!("CIRCUIT BREAKER STOP: {}", trigger.message);
                    }
                    BreakerSeverity::Pause => {
                        warn!("CIRCUIT BREAKER PAUSE: {}", trigger.message);
                    }
                    BreakerSeverity::Throttle => {
                        warn!("CIRCUIT BREAKER THROTTLE: {}", trigger.message);
                    }
                    BreakerSeverity::Warning => {
                        warn!("CIRCUIT BREAKER WARNING: {}", trigger.message);
                    }
                }

                // Start recovery if auto-recovery enabled
                if breaker.config.auto_recovery {
                    breaker.state = BreakerState::Recovery;
                    breaker.recovery_start_time = Some(now);
                }

                triggers.push(trigger.clone());

                // Collect cascade targets before releasing the lock
                let cascade_targets = breaker.config.cascading_impact.clone();

                // Add to recent triggers
                self.recent_triggers.push_back(trigger.clone());
                while self.recent_triggers.len() > self.max_trigger_history {
                    self.recent_triggers.pop_front();
                }

                // Release the lock before handling cascades
                drop(breakers);

                // Handle cascading triggers after releasing the lock
                for cascade_breaker in &cascade_targets {
                    self.trigger_cascade(cascade_breaker, &trigger).await?;
                }
            }
        }

        Ok(triggers)
    }

    async fn trigger_cascade(
        &mut self,
        cascade_target: &str,
        source_trigger: &BreakerTrigger,
    ) -> Result<()> {
        // Artificial delay to prevent cascade loops
        tokio::time::sleep(tokio::time::Duration::from_secs(self.cascade_delay_seconds)).await;

        match cascade_target {
            "emergency_liquidation" => {
                *self.global_emergency_stop.write().await = true;
                error!(
                    "CASCADE: Emergency liquidation triggered by {}",
                    source_trigger.breaker_name
                );
            }
            "all_strategies_pause" => {
                warn!(
                    "CASCADE: All strategies paused by {}",
                    source_trigger.breaker_name
                );
                // In real implementation, would signal strategy manager
            }
            "reduce_position_sizes" => {
                warn!(
                    "CASCADE: Position size reduction triggered by {}",
                    source_trigger.breaker_name
                );
                // In real implementation, would signal position sizer
            }
            "diversification_required" => {
                warn!(
                    "CASCADE: Diversification required by {}",
                    source_trigger.breaker_name
                );
                // In real implementation, would signal portfolio rebalancer
            }
            _ => {
                // Try to trigger another named breaker
                if self.breakers.read().await.get(cascade_target).is_some() {
                    debug!(
                        "CASCADE: Triggering breaker {} from {}",
                        cascade_target, source_trigger.breaker_name
                    );
                    // Recursive cascade - would implement carefully in production
                }
            }
        }

        Ok(())
    }

    pub async fn is_emergency_stop_active(&self) -> bool {
        *self.global_emergency_stop.read().await
    }

    pub async fn clear_emergency_stop(&mut self) -> Result<()> {
        *self.global_emergency_stop.write().await = false;
        info!("Emergency stop cleared manually");
        Ok(())
    }

    pub async fn disable_breaker(&mut self, breaker_name: &str) -> Result<()> {
        if let Some(breaker) = self.breakers.write().await.get_mut(breaker_name) {
            breaker.state = BreakerState::Disabled;
            info!("Circuit breaker '{}' disabled", breaker_name);
        }
        Ok(())
    }

    pub async fn enable_breaker(&mut self, breaker_name: &str) -> Result<()> {
        if let Some(breaker) = self.breakers.write().await.get_mut(breaker_name) {
            breaker.state = BreakerState::Armed;
            info!("Circuit breaker '{}' enabled", breaker_name);
        }
        Ok(())
    }

    pub async fn get_breaker_status(&self) -> HashMap<String, BreakerMetrics> {
        let breakers = self.breakers.read().await;
        let mut status = HashMap::new();
        let now = Utc::now();

        for (name, breaker) in breakers.iter() {
            let time_since_trigger = breaker
                .last_trigger_time
                .map(|t| now.signed_duration_since(t).num_minutes());

            let utilization = if breaker.config.threshold != 0.0 {
                (breaker.current_value / breaker.config.threshold.abs() * 100.0).min(100.0)
            } else {
                0.0
            };

            let metrics = BreakerMetrics {
                current_value: breaker.current_value,
                threshold: breaker.config.threshold,
                utilization_pct: utilization,
                triggers_today: breaker.trigger_count_day,
                last_trigger: breaker.last_trigger_time,
                time_since_last_trigger_minutes: time_since_trigger,
                state: breaker.state.clone(),
            };

            status.insert(name.clone(), metrics);
        }

        status
    }

    pub async fn get_recent_triggers(&self, limit: usize) -> Vec<BreakerTrigger> {
        self.recent_triggers
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    pub async fn generate_status_report(&self) -> String {
        let status = self.get_breaker_status().await;
        let emergency_active = self.is_emergency_stop_active().await;
        let recent_triggers = self.get_recent_triggers(10).await;

        let mut report = String::new();
        report.push_str("=== Advanced Circuit Breaker Status ===\n\n");

        if emergency_active {
            report.push_str("🚨 EMERGENCY STOP ACTIVE 🚨\n\n");
        }

        report.push_str("📊 Breaker Status:\n");
        for (name, metrics) in status {
            let status_icon = match metrics.state {
                BreakerState::Armed => "✅",
                BreakerState::Triggered => "🔴",
                BreakerState::Recovery => "🟡",
                BreakerState::Disabled => "⚫",
            };

            report.push_str(&format!(
                "{} {}: {:.2}/{:.2} ({:.1}%) - {} triggers today\n",
                status_icon,
                name,
                metrics.current_value,
                metrics.threshold,
                metrics.utilization_pct,
                metrics.triggers_today
            ));
        }

        if !recent_triggers.is_empty() {
            report.push_str("\n🚨 Recent Triggers:\n");
            for trigger in recent_triggers.iter().take(5) {
                report.push_str(&format!(
                    "• {}: {} ({:.2} > {:.2}) - {}\n",
                    trigger.timestamp.format("%H:%M:%S"),
                    trigger.breaker_name,
                    trigger.trigger_value,
                    trigger.threshold,
                    match trigger.severity {
                        BreakerSeverity::Emergency => "EMERGENCY",
                        BreakerSeverity::Stop => "STOP",
                        BreakerSeverity::Pause => "PAUSE",
                        BreakerSeverity::Throttle => "THROTTLE",
                        BreakerSeverity::Warning => "WARNING",
                    }
                ));
            }
        }

        report.push_str("\nSystem State:\n");
        report.push_str(&format!(
            "• Portfolio Value: ${:.2}\n",
            self.portfolio_value
        ));
        report.push_str(&format!(
            "• Portfolio Drawdown: {:.2}%\n",
            self.portfolio_drawdown
        ));
        report.push_str(&format!(
            "• Market Volatility: {:.1}%\n",
            self.market_volatility * 100.0
        ));
        report.push_str(&format!(
            "• System Correlation: {:.2}\n",
            self.system_correlation
        ));

        report
    }
}
