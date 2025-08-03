use shared_models::error::Result;
use tracing::{info, debug};
use chrono::{DateTime, Utc, Duration};
use std::collections::{HashMap, BTreeMap, VecDeque};
use serde::{Serialize, Deserialize};
use tokio::sync::RwLock;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardMetrics {
    pub total_pnl_usd: f64,
    pub daily_pnl_usd: f64,
    pub unrealized_pnl_usd: f64,
    pub total_volume_usd: f64,
    pub daily_volume_usd: f64,
    pub win_rate: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown_pct: f64,
    pub current_drawdown_pct: f64,
    pub active_positions: u32,
    pub total_trades: u64,
    pub daily_trades: u32,
    pub avg_trade_duration_minutes: f64,
    pub best_performing_strategy: String,
    pub worst_performing_strategy: String,
    pub risk_utilization_pct: f64,
    pub correlation_exposure: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyMetrics {
    pub strategy_id: String,
    pub strategy_type: String,
    pub pnl_usd: f64,
    pub daily_pnl_usd: f64,
    pub volume_usd: f64,
    pub win_rate: f64,
    pub sharpe_ratio: f64,
    pub trades_count: u32,
    pub avg_trade_size_usd: f64,
    pub max_drawdown_pct: f64,
    pub current_allocation_pct: f64,
    pub risk_score: f64,
    pub last_trade_time: Option<DateTime<Utc>>,
    pub active_positions: u32,
    pub success_rate_24h: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetrics {
    pub portfolio_var_95: f64,
    pub portfolio_var_99: f64,
    pub portfolio_volatility: f64,
    pub correlation_risk_score: f64,
    pub concentration_risk_score: f64,
    pub liquidity_risk_score: f64,
    pub max_position_size_pct: f64,
    pub current_leverage: f64,
    pub margin_utilization_pct: f64,
    pub circuit_breaker_triggered: bool,
    pub last_risk_assessment: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketMetrics {
    pub total_market_cap_usd: f64,
    pub active_tokens: u32,
    pub avg_volatility_pct: f64,
    pub market_momentum_score: f64,
    pub volume_surge_indicators: u32,
    pub social_sentiment_score: f64,
    pub bridge_flow_score: f64,
    pub whale_activity_score: f64,
    pub market_regime: String, // "Bull", "Bear", "Sideways", "High Vol"
    pub opportunity_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub secondary_value: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub overview: DashboardMetrics,
    pub strategies: Vec<StrategyMetrics>,
    pub risk: RiskMetrics,
    pub market: MarketMetrics,
    
    // Time series data for charts (last 24 hours)
    pub pnl_timeseries: Vec<TimeSeriesPoint>,
    pub volume_timeseries: Vec<TimeSeriesPoint>,
    pub drawdown_timeseries: Vec<TimeSeriesPoint>,
    pub risk_timeseries: Vec<TimeSeriesPoint>,
    pub opportunity_timeseries: Vec<TimeSeriesPoint>,
    
    // Recent activity
    pub recent_trades: Vec<TradeActivity>,
    pub recent_alerts: Vec<AlertActivity>,
    pub recent_opportunities: Vec<OpportunityActivity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeActivity {
    pub timestamp: DateTime<Utc>,
    pub strategy_id: String,
    pub symbol: String,
    pub side: String,
    pub size_usd: f64,
    pub price: f64,
    pub pnl_usd: Option<f64>,
    pub status: String, // "Executed", "Partial", "Failed"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertActivity {
    pub timestamp: DateTime<Utc>,
    pub level: String, // "Info", "Warning", "Critical"
    pub category: String, // "Risk", "Performance", "System", "Market"
    pub message: String,
    pub strategy_id: Option<String>,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpportunityActivity {
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub opportunity_type: String,
    pub confidence_score: f64,
    pub potential_return_pct: f64,
    pub risk_score: f64,
    pub strategies_interested: Vec<String>,
    pub status: String, // "Detected", "Evaluated", "Executed", "Missed"
}

pub struct PerformanceDashboard {
    dashboard_data: Arc<RwLock<DashboardData>>,
    
    // Historical data storage
    historical_metrics: HashMap<String, VecDeque<TimeSeriesPoint>>,
    trade_history: VecDeque<TradeActivity>,
    alert_history: VecDeque<AlertActivity>,
    opportunity_history: VecDeque<OpportunityActivity>,
    
    // Configuration
    max_history_points: usize,
    max_activity_records: usize,
    update_interval_seconds: u64,
    
    // State tracking
    last_portfolio_value: f64,
    session_start_time: DateTime<Utc>,
    peak_portfolio_value: f64,
}

impl PerformanceDashboard {
    pub fn new() -> Self {
        let initial_data = DashboardData {
            overview: DashboardMetrics {
                total_pnl_usd: 0.0,
                daily_pnl_usd: 0.0,
                unrealized_pnl_usd: 0.0,
                total_volume_usd: 0.0,
                daily_volume_usd: 0.0,
                win_rate: 0.0,
                sharpe_ratio: 0.0,
                max_drawdown_pct: 0.0,
                current_drawdown_pct: 0.0,
                active_positions: 0,
                total_trades: 0,
                daily_trades: 0,
                avg_trade_duration_minutes: 0.0,
                best_performing_strategy: "None".to_string(),
                worst_performing_strategy: "None".to_string(),
                risk_utilization_pct: 0.0,
                correlation_exposure: 0.0,
                timestamp: Utc::now(),
            },
            strategies: Vec::new(),
            risk: RiskMetrics {
                portfolio_var_95: 0.0,
                portfolio_var_99: 0.0,
                portfolio_volatility: 0.0,
                correlation_risk_score: 0.0,
                concentration_risk_score: 0.0,
                liquidity_risk_score: 0.0,
                max_position_size_pct: 10.0,
                current_leverage: 1.0,
                margin_utilization_pct: 0.0,
                circuit_breaker_triggered: false,
                last_risk_assessment: Utc::now(),
            },
            market: MarketMetrics {
                total_market_cap_usd: 0.0,
                active_tokens: 0,
                avg_volatility_pct: 0.0,
                market_momentum_score: 0.0,
                volume_surge_indicators: 0,
                social_sentiment_score: 0.0,
                bridge_flow_score: 0.0,
                whale_activity_score: 0.0,
                market_regime: "Unknown".to_string(),
                opportunity_count: 0,
            },
            pnl_timeseries: Vec::new(),
            volume_timeseries: Vec::new(),
            drawdown_timeseries: Vec::new(),
            risk_timeseries: Vec::new(),
            opportunity_timeseries: Vec::new(),
            recent_trades: Vec::new(),
            recent_alerts: Vec::new(),
            recent_opportunities: Vec::new(),
        };

        Self {
            dashboard_data: Arc::new(RwLock::new(initial_data)),
            historical_metrics: HashMap::new(),
            trade_history: VecDeque::new(),
            alert_history: VecDeque::new(),
            opportunity_history: VecDeque::new(),
            max_history_points: 1440, // 24 hours at 1-minute intervals
            max_activity_records: 1000,
            update_interval_seconds: 30,
            last_portfolio_value: 200.0, // Starting with $200
            session_start_time: Utc::now(),
            peak_portfolio_value: 200.0,
        }
    }

    pub async fn update_portfolio_metrics(
        &mut self,
        total_pnl: f64,
        unrealized_pnl: f64,
        daily_volume: f64,
        active_positions: u32,
        total_trades: u64,
    ) -> Result<()> {
        let now = Utc::now();
        let current_value = 200.0 + total_pnl; // Starting portfolio + PnL
        
        // Update peak value
        if current_value > self.peak_portfolio_value {
            self.peak_portfolio_value = current_value;
        }
        
        // Calculate current drawdown
        let current_drawdown = if self.peak_portfolio_value > 0.0 {
            ((self.peak_portfolio_value - current_value) / self.peak_portfolio_value) * 100.0
        } else { 0.0 };
        
        // Calculate daily P&L
        let session_duration = now.signed_duration_since(self.session_start_time);
        let daily_pnl = if session_duration.num_hours() >= 24 {
            // Calculate 24h rolling P&L
            self.calculate_daily_pnl(now).await
        } else {
            total_pnl // Session P&L for shorter periods
        };
        
        // Update data - release lock before adding timeseries
        {
            let mut data = self.dashboard_data.write().await;
            
            // Update overview metrics
            data.overview.total_pnl_usd = total_pnl;
            data.overview.daily_pnl_usd = daily_pnl;
            data.overview.unrealized_pnl_usd = unrealized_pnl;
            data.overview.daily_volume_usd = daily_volume;
            data.overview.active_positions = active_positions;
            data.overview.total_trades = total_trades;
            data.overview.current_drawdown_pct = current_drawdown;
            data.overview.timestamp = now;
        }
        
        // Add to time series after releasing the lock
        self.add_timeseries_point("pnl", now, total_pnl).await;
        self.add_timeseries_point("portfolio_value", now, current_value).await;
        
        info!("Updated portfolio metrics - P&L: ${:.2}, Value: ${:.2}, Drawdown: {:.2}%", 
              total_pnl, current_value, current_drawdown);
        
        Ok(())
    }

    pub async fn update_strategy_metrics(
        &mut self,
        strategy_metrics: Vec<StrategyMetrics>,
    ) -> Result<()> {
        let mut data = self.dashboard_data.write().await;
        
        // Find best and worst performing strategies
        let mut best_strategy = "None".to_string();
        let mut worst_strategy = "None".to_string();
        let mut best_pnl = f64::NEG_INFINITY;
        let mut worst_pnl = f64::INFINITY;
        
        // Calculate aggregate metrics
        let mut total_strategy_volume = 0.0;
        let mut total_strategy_trades = 0;
        let mut weighted_win_rate = 0.0;
        let mut total_weight = 0.0;
        
        for strategy in &strategy_metrics {
            // Track best/worst performers
            if strategy.pnl_usd > best_pnl {
                best_pnl = strategy.pnl_usd;
                best_strategy = strategy.strategy_id.clone();
            }
            if strategy.pnl_usd < worst_pnl {
                worst_pnl = strategy.pnl_usd;
                worst_strategy = strategy.strategy_id.clone();
            }
            
            // Aggregate metrics
            total_strategy_volume += strategy.volume_usd;
            total_strategy_trades += strategy.trades_count;
            
            // Weight win rate by volume
            if strategy.volume_usd > 0.0 {
                weighted_win_rate += strategy.win_rate * strategy.volume_usd;
                total_weight += strategy.volume_usd;
            }
        }
        
        // Update overview with strategy-derived metrics
        data.overview.total_volume_usd = total_strategy_volume;
        data.overview.daily_trades = total_strategy_trades;
        data.overview.win_rate = if total_weight > 0.0 { weighted_win_rate / total_weight } else { 0.0 };
        data.overview.best_performing_strategy = best_strategy;
        data.overview.worst_performing_strategy = worst_strategy;
        
        // Update strategy list
        data.strategies = strategy_metrics;
        
        Ok(())
    }

    pub async fn update_risk_metrics(&mut self, risk_metrics: RiskMetrics) -> Result<()> {
        let now = Utc::now();
        let var_95 = risk_metrics.portfolio_var_95;
        let volatility = risk_metrics.portfolio_volatility;
        
        // Update data - release lock before adding timeseries
        {
            let mut data = self.dashboard_data.write().await;
            data.risk = risk_metrics;
            
            // Update overview risk utilization
            data.overview.risk_utilization_pct = data.risk.margin_utilization_pct;
            data.overview.correlation_exposure = data.risk.correlation_risk_score;
        }
        
        // Add to risk time series after releasing the lock
        self.add_timeseries_point("var_95", now, var_95).await;
        self.add_timeseries_point("volatility", now, volatility).await;
        
        Ok(())
    }

    pub async fn update_market_metrics(&mut self, market_metrics: MarketMetrics) -> Result<()> {
        let now = Utc::now();
        let opportunity_count = market_metrics.opportunity_count as f64;
        let sentiment_score = market_metrics.social_sentiment_score;
        
        // Update data - release lock before adding timeseries  
        {
            let mut data = self.dashboard_data.write().await;
            data.market = market_metrics;
        }
        
        // Add to opportunity time series after releasing the lock
        self.add_timeseries_point("opportunities", now, opportunity_count).await;
        self.add_timeseries_point("sentiment", now, sentiment_score).await;
        
        Ok(())
    }

    pub async fn record_trade_activity(&mut self, trade: TradeActivity) -> Result<()> {
        // Add to history
        self.trade_history.push_back(trade.clone());
        
        // Maintain size limit
        while self.trade_history.len() > self.max_activity_records {
            self.trade_history.pop_front();
        }
        
        // Update dashboard data
        let mut data = self.dashboard_data.write().await;
        data.recent_trades.push(trade);
        
        // Keep only recent trades (last 100)
        if data.recent_trades.len() > 100 {
            let excess = data.recent_trades.len() - 100;
            data.recent_trades.drain(0..excess);
        }
        
        Ok(())
    }

    pub async fn record_alert(&mut self, alert: AlertActivity) -> Result<()> {
        // Add to history
        self.alert_history.push_back(alert.clone());
        
        // Maintain size limit
        while self.alert_history.len() > self.max_activity_records {
            self.alert_history.pop_front();
        }
        
        // Update dashboard data
        let mut data = self.dashboard_data.write().await;
        data.recent_alerts.push(alert);
        
        // Keep only recent alerts (last 50)
        if data.recent_alerts.len() > 50 {
            let excess = data.recent_alerts.len() - 50;
            data.recent_alerts.drain(0..excess);
        }
        
        Ok(())
    }

    pub async fn record_opportunity(&mut self, opportunity: OpportunityActivity) -> Result<()> {
        // Add to history
        self.opportunity_history.push_back(opportunity.clone());
        
        // Maintain size limit
        while self.opportunity_history.len() > self.max_activity_records {
            self.opportunity_history.pop_front();
        }
        
        // Update dashboard data
        let mut data = self.dashboard_data.write().await;
        data.recent_opportunities.push(opportunity);
        
        // Keep only recent opportunities (last 50)
        if data.recent_opportunities.len() > 50 {
            let excess = data.recent_opportunities.len() - 50;
            data.recent_opportunities.drain(0..excess);
        }
        
        Ok(())
    }

    pub async fn get_dashboard_data(&self) -> DashboardData {
        self.dashboard_data.read().await.clone()
    }

    pub async fn get_dashboard_json(&self) -> Result<String> {
        let data = self.get_dashboard_data().await;
        Ok(serde_json::to_string_pretty(&data)?)
    }

    // Helper method to add time series points
    async fn add_timeseries_point(&mut self, metric: &str, timestamp: DateTime<Utc>, value: f64) {
        let point = TimeSeriesPoint {
            timestamp,
            value,
            secondary_value: None,
        };
        
        let series = self.historical_metrics
            .entry(metric.to_string())
            .or_insert_with(VecDeque::new);
        
        series.push_back(point.clone());
        
        // Maintain size limit
        while series.len() > self.max_history_points {
            series.pop_front();
        }
        
        // Update dashboard data with recent points
        let mut data = self.dashboard_data.write().await;
        match metric {
            "pnl" => data.pnl_timeseries = series.iter().cloned().collect(),
            "var_95" => data.risk_timeseries = series.iter().cloned().collect(),
            "opportunities" => data.opportunity_timeseries = series.iter().cloned().collect(),
            _ => {}
        }
    }

    // Calculate daily P&L from historical data
    async fn calculate_daily_pnl(&self, current_time: DateTime<Utc>) -> f64 {
        let twenty_four_hours_ago = current_time - Duration::hours(24);
        
        if let Some(pnl_series) = self.historical_metrics.get("pnl") {
            // Find the P&L value from 24 hours ago
            if let Some(old_point) = pnl_series.iter()
                .find(|p| p.timestamp >= twenty_four_hours_ago) {
                
                if let Some(current_point) = pnl_series.back() {
                    return current_point.value - old_point.value;
                }
            }
        }
        
        // Fallback to session P&L if no historical data
        0.0
    }

    pub async fn calculate_sharpe_ratio(&self, periods: usize) -> f64 {
        if let Some(pnl_series) = self.historical_metrics.get("pnl") {
            if pnl_series.len() < 2 || periods < 2 {
                return 0.0;
            }
            
            let recent_points: Vec<_> = pnl_series.iter()
                .rev()
                .take(periods)
                .collect();
            
            if recent_points.len() < 2 {
                return 0.0;
            }
            
            // Calculate returns
            let mut returns = Vec::new();
            for i in 1..recent_points.len() {
                let current_val = recent_points[i-1].value;
                let prev_val = recent_points[i].value;
                if prev_val != 0.0 {
                    returns.push((current_val - prev_val) / prev_val.abs());
                }
            }
            
            if returns.is_empty() {
                return 0.0;
            }
            
            // Calculate mean and std dev
            let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
            let variance = returns.iter()
                .map(|r| (r - mean_return).powi(2))
                .sum::<f64>() / returns.len() as f64;
            let std_dev = variance.sqrt();
            
            if std_dev == 0.0 {
                0.0
            } else {
                // Annualized Sharpe (assuming risk-free rate of 0)
                mean_return / std_dev * (252.0_f64).sqrt() // 252 trading days
            }
        } else {
            0.0
        }
    }

    // Generate performance summary for specific time period
    pub async fn generate_performance_summary(&self, hours_back: i64) -> String {
        let data = self.get_dashboard_data().await;
        let sharpe = self.calculate_sharpe_ratio(hours_back as usize * 2).await; // 30-min intervals
        
        format!(
            "=== MemeSnipe v25 Performance Dashboard ===\n\
            📊 Portfolio Overview:\n\
            • Total P&L: ${:.2}\n\
            • Daily P&L: ${:.2}\n\
            • Current Value: ${:.2}\n\
            • Drawdown: {:.2}%\n\
            • Win Rate: {:.1}%\n\
            • Sharpe Ratio: {:.2}\n\
            \n\
            🎯 Active Trading:\n\
            • Positions: {}\n\
            • Daily Trades: {}\n\
            • Volume: ${:.0}\n\
            • Best Strategy: {}\n\
            \n\
            ⚠️  Risk Metrics:\n\
            • VaR 95%: ${:.2}\n\
            • Portfolio Vol: {:.1}%\n\
            • Risk Utilization: {:.1}%\n\
            • Circuit Breaker: {}\n\
            \n\
            🌍 Market Conditions:\n\
            • Regime: {}\n\
            • Opportunities: {}\n\
            • Sentiment: {:.1}/10\n\
            • Active Tokens: {}\n\
            \n\
            Updated: {}",
            data.overview.total_pnl_usd,
            data.overview.daily_pnl_usd,
            200.0 + data.overview.total_pnl_usd,
            data.overview.current_drawdown_pct,
            data.overview.win_rate * 100.0,
            sharpe,
            data.overview.active_positions,
            data.overview.daily_trades,
            data.overview.daily_volume_usd,
            data.overview.best_performing_strategy,
            data.risk.portfolio_var_95,
            data.risk.portfolio_volatility * 100.0,
            data.overview.risk_utilization_pct,
            if data.risk.circuit_breaker_triggered { "TRIGGERED" } else { "Normal" },
            data.market.market_regime,
            data.market.opportunity_count,
            data.market.social_sentiment_score,
            data.market.active_tokens,
            data.overview.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
        )
    }
}
