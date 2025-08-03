use anyhow::Result;
use shared_models::{PortfolioRiskMetrics, RiskLevel, Trade, Side, RiskEvent};
use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};

pub struct RiskManager {
    max_position_size: f64,
    max_daily_loss: f64,
    max_portfolio_exposure: f64,
    daily_pnl: f64,
    position_sizes: HashMap<String, f64>,
}

impl RiskManager {
    pub fn new() -> Self {
        Self {
            max_position_size: 50.0, // $50 max per position
            max_daily_loss: 20.0,    // $20 max daily loss
            max_portfolio_exposure: 100.0, // $100 max total exposure
            daily_pnl: 0.0,
            position_sizes: HashMap::new(),
        }
    }

    pub async fn validate_trade(&self, trade: &Trade) -> Result<bool, anyhow::Error> {
        let current_exposure: f64 = self.position_sizes.values().sum();
        let portfolio_utilization = current_exposure / self.max_portfolio_exposure;
        
        if portfolio_utilization > self.max_portfolio_exposure {
            return Ok(false);
        }
        
        let position_exposure = self.position_sizes
            .get(&trade.symbol)
            .unwrap_or(&0.0);
            
        if *position_exposure + (trade.quantity * trade.price) > self.max_position_size {
            return Ok(false);
        }
        
        Ok(true)
    }

    pub async fn update_position(&mut self, trade: &Trade) -> Result<(), anyhow::Error> {
        let position_exposure = self.position_sizes
            .entry(trade.symbol.clone())
            .or_insert(0.0);
        
        let trade_value = trade.quantity * trade.price;
        
        match trade.side {
            Side::Long => *position_exposure += trade_value,
            Side::Short => *position_exposure -= trade_value,
        }
        
        if *position_exposure <= 0.0 {
            self.position_sizes.remove(&trade.symbol);
        }
        
        Ok(())
    }

    pub fn update_daily_pnl(&mut self, pnl_change: f64) {
        self.daily_pnl += pnl_change;
    }

    pub fn get_risk_metrics(&self) -> PortfolioRiskMetrics {
        let current_exposure: f64 = self.position_sizes.values().sum();
        let portfolio_utilization = current_exposure / self.max_portfolio_exposure;
        
        PortfolioRiskMetrics {
            portfolio_value: 200.0 + self.daily_pnl,
            daily_pnl: self.daily_pnl,
            max_drawdown: self.daily_pnl.min(0.0),
            exposure_percentage: portfolio_utilization * 100.0,
            var_95: self.daily_pnl * 1.65, // Simple VaR approximation
            position_count: self.position_sizes.len() as u32,
            risk_score: if portfolio_utilization > 0.8 { 
                RiskLevel::High 
            } else if portfolio_utilization > 0.5 { 
                RiskLevel::Medium 
            } else { 
                RiskLevel::Low 
            },
        }
    }

    pub fn reset_daily_metrics(&mut self) {
        self.daily_pnl = 0.0;
        tracing::info!("📊 Daily risk metrics reset");
    }
}
