use crate::indicators::{Indicator, IndicatorConfig};
use crate::signals::StrategySignal;
use anyhow::{anyhow, Result};
use std::collections::HashMap;

pub struct VolumeAnalyzer<'a> {
    pub config: &'a IndicatorConfig,
    pub volume_history: &'a HashMap<String, Vec<Indicator>>,
    pub price_history: &'a HashMap<String, Vec<Indicator>>,
}

impl<'a> VolumeAnalyzer<'a> {
    pub fn new(
        config: &'a IndicatorConfig,
        volume_history: &'a HashMap<String, Vec<Indicator>>,
        price_history: &'a HashMap<String, Vec<Indicator>>,
    ) -> Self {
        Self {
            config,
            volume_history,
            price_history,
        }
    }

    async fn analyze(&mut self, token: &str) -> Result<Option<StrategySignal>> {
        let history = self.volume_history
            .get(token)
            .ok_or_else(|| anyhow::anyhow!("No volume history for token"))?;
        
        if history.len() < self.config.min_data_points {
            return Ok(None);
        }
        
        // Calculate average volume
        let avg_volume: f64 = history.iter().map(|v| v.volume).sum::<f64>() / history.len() as f64;
        
        let latest = history.last().ok_or_else(|| {
            anyhow::anyhow!("No latest volume in history")
        })?;
        
        let spike_ratio = latest.volume / avg_volume;
        
        // Check if this is a significant spike
        if spike_ratio < self.config.min_spike_ratio {
            return Ok(None);
        }
        
        // Calculate price range
        let price_history = self.price_history
            .get(token)
            .ok_or_else(|| anyhow::anyhow!("No price history for token"))?;
            
        let high = price_history
            .iter()
            .max_by(|a, b| a.price.partial_cmp(&b.price).unwrap_or(std::cmp::Ordering::Equal))
            .ok_or_else(|| anyhow::anyhow!("No high price in history"))?
            .price;
        
        let low = price_history
            .iter()
            .min_by(|a, b| a.price.partial_cmp(&b.price).unwrap_or(std::cmp::Ordering::Equal))
            .ok_or_else(|| anyhow::anyhow!("No low price in history"))?
            .price;

        // ...existing code...

        Ok(None)
    }
}