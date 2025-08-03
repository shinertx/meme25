use crate::strategies::StrategySignal;
use metrics::{counter, gauge, histogram};

pub struct Metrics;

impl Metrics {
    pub async fn record_trade(&self, signal: &StrategySignal, outcome: &TradeOutcome) {
        // Record comprehensive metrics
        counter!("trades_total", 1.0, 
            "strategy" => signal.strategy_name.clone(),
            "action" => signal.action.to_string(),
            "status" => outcome.status.to_string()
        );
        
        if let Some(pnl) = outcome.realized_pnl {
            histogram!("trade_pnl_usd", pnl,
                "strategy" => signal.strategy_name.clone()
            );
            
            // Alert on big wins/losses
            if pnl.abs() > 100.0 {
                gauge!("large_pnl_alert", pnl,
                    "strategy" => signal.strategy_name.clone(),
                    "direction" => if pnl > 0.0 { "profit" } else { "loss" }
                );
            }
        }
        
        if let Some(slippage) = outcome.slippage_bps {
            histogram!("execution_slippage_bps", slippage as f64,
                "strategy" => signal.strategy_name.clone()
            );
        }
        
        // Portfolio metrics
        gauge!("portfolio_value_usd", outcome.portfolio_value);
        gauge!("open_positions", outcome.open_positions as f64);
        
        // Strategy-specific metrics
        self.update_strategy_performance(&signal.strategy_name, outcome).await;
    }
    
    async fn update_strategy_performance(&self, strategy: &str, outcome: &TradeOutcome) {
        let win_rate = outcome.strategy_stats.win_rate;
        let sharpe = outcome.strategy_stats.sharpe_ratio;
        
        gauge!("strategy_win_rate", win_rate,
            "strategy" => strategy.to_string()
        );
        
        gauge!("strategy_sharpe_ratio", sharpe,
            "strategy" => strategy.to_string()
        );
        
        // Alert if strategy is underperforming
        if win_rate < 0.3 || sharpe < 0.0 {
            counter!("strategy_underperformance_alert", 1.0,
                "strategy" => strategy.to_string(),
                "win_rate" => win_rate.to_string(),
                "sharpe" => sharpe.to_string()
            );
        }
    }
}

pub struct TradeOutcome {
    pub status: TradeStatus,
    pub realized_pnl: Option<f64>,
    pub slippage_bps: Option<u16>,
    pub portfolio_value: f64,
    pub open_positions: usize,
    pub strategy_stats: StrategyStats,
}

pub struct StrategyStats {
    pub win_rate: f64,
    pub sharpe_ratio: f64,
}

pub enum TradeStatus {
    Executed,
    Rejected,
    Failed,
}

impl std::fmt::Display for TradeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TradeStatus::Executed => write!(f, "executed"),
            TradeStatus::Rejected => write!(f, "rejected"),
            TradeStatus::Failed => write!(f, "failed"),
        }
    }
}