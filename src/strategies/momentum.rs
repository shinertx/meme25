use crate::prelude::*;
use crate::strategies::StrategySignal;
use crate::types::{PriceData, StrategyConfig};
use std::collections::HashMap;

pub struct MomentumAnalyzer<'a> {
    config: &'a StrategyConfig,
    price_history: &'a mut HashMap<String, Vec<PriceData>>,
}

impl<'a> MomentumAnalyzer<'a> {
    pub fn new(config: &'a StrategyConfig, price_history: &'a mut HashMap<String, Vec<PriceData>>) -> Self {
        MomentumAnalyzer { config, price_history }
    }
    
    async fn analyze(&mut self, token: &str) -> Result<Option<StrategySignal>> {
        let history = self.price_history
            .get(token)
            .ok_or_else(|| anyhow::anyhow!("No price history for token"))?;
        
        if history.len() < self.config.min_data_points {
            return Ok(None);
        }
        
        // Calculate momentum
        if history.len() < 2 {
            return Ok(None);
        }
        
        let latest = history.last().ok_or_else(|| {
            anyhow::anyhow!("No latest price in history")
        })?;
        let oldest = history.first().ok_or_else(|| {
            anyhow::anyhow!("No oldest price in history")
        })?;
        
        let momentum = (latest.price - oldest.price) / oldest.price;
        
        // ...existing code...
    }
}