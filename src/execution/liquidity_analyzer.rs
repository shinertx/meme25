use anyhow::{Context, Result};
use std::collections::HashMap;

pub struct LiquidityAnalyzer {
    min_liquidity_usd: f64,
    max_price_impact: f64,
    max_slippage_bps: u16,
}

impl LiquidityAnalyzer {
    pub fn new() -> Self {
        Self {
            min_liquidity_usd: 10_000.0,  // $10k minimum liquidity
            max_price_impact: 0.02,        // 2% max price impact
            max_slippage_bps: 100,         // 1% max slippage
        }
    }
    
    pub async fn check_tradeable(&self, token: &str, amount_usd: f64) -> Result<TradingViability> {
        // Get liquidity data from multiple sources
        let pool_data = self.get_pool_data(token).await?;
        
        // Check minimum liquidity
        if pool_data.liquidity_usd < self.min_liquidity_usd {
            return Ok(TradingViability::NotViable {
                reason: format!("Insufficient liquidity: ${:.2} < ${:.2}", 
                    pool_data.liquidity_usd, self.min_liquidity_usd)
            });
        }
        
        // Calculate price impact
        let price_impact = self.calculate_price_impact(
            amount_usd,
            pool_data.liquidity_usd,
            pool_data.fee_bps
        );
        
        if price_impact > self.max_price_impact {
            return Ok(TradingViability::NotViable {
                reason: format!("Price impact too high: {:.2}% > {:.2}%", 
                    price_impact * 100.0, self.max_price_impact * 100.0)
            });
        }
        
        // Check if token is rugpull risk
        if pool_data.lp_burned_percent < 90.0 {
            return Ok(TradingViability::Risky {
                warnings: vec![
                    format!("LP not fully burned: {:.1}%", pool_data.lp_burned_percent)
                ],
                suggested_size_reduction: 0.5,  // Trade 50% of intended size
            });
        }
        
        Ok(TradingViability::Viable {
            expected_slippage_bps: (price_impact * 10000.0) as u16,
            available_liquidity_usd: pool_data.liquidity_usd,
        })
    }
    
    async fn get_pool_data(&self, token: &str) -> Result<PoolData> {
        // TODO: Integrate with Birdeye/Jupiter APIs
        // For now, return mock data for paper trading
        Ok(PoolData {
            liquidity_usd: 50_000.0,
            fee_bps: 30,
            lp_burned_percent: 100.0,
        })
    }
    
    fn calculate_price_impact(&self, trade_size: f64, liquidity: f64, fee_bps: u16) -> f64 {
        // Simplified constant product AMM formula
        let fee_multiplier = 1.0 - (fee_bps as f64 / 10000.0);
        let size_ratio = trade_size / liquidity;
        
        // Price impact approximation
        size_ratio / (1.0 - size_ratio) * fee_multiplier
    }
}

pub enum TradingViability {
    Viable {
        expected_slippage_bps: u16,
        available_liquidity_usd: f64,
    },
    Risky {
        warnings: Vec<String>,
        suggested_size_reduction: f64,
    },
    NotViable {
        reason: String,
    },
}

struct PoolData {
    liquidity_usd: f64,
    fee_bps: u16,
    lp_burned_percent: f64,
}
