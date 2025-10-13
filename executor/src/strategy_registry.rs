use async_trait::async_trait;
use shared_models::error::{ModelError, Result};
use shared_models::{Event, StrategyType};
use std::collections::HashMap;
use tracing::{error, info};

#[async_trait]
pub trait StrategyTrait: Send + Sync {
    async fn process_event(&mut self, event: &Event) -> Result<()>;
    fn get_type(&self) -> StrategyType;
    fn is_active(&self) -> bool;
}

pub struct StrategyRegistry {
    strategies: HashMap<StrategyType, Box<dyn StrategyTrait>>,
}

impl StrategyRegistry {
    pub fn new() -> Self {
        Self {
            strategies: HashMap::new(),
        }
    }

    pub fn register_strategy(&mut self, strategy: Box<dyn StrategyTrait>) {
        let strategy_type = strategy.get_type();
        info!("Registering strategy: {:?}", strategy_type);
        self.strategies.insert(strategy_type, strategy);
    }

    pub async fn process_event(&mut self, event: &Event) -> Result<()> {
        let mut processed_count = 0;

        for (strategy_type, strategy) in &mut self.strategies {
            if strategy.is_active() {
                match strategy.process_event(event).await {
                    Ok(()) => {
                        processed_count += 1;
                    }
                    Err(e) => {
                        error!(
                            "Strategy {:?} failed to process event: {}",
                            strategy_type, e
                        );
                        // Continue processing other strategies even if one fails
                    }
                }
            }
        }

        if processed_count == 0 {
            return Err(ModelError::Strategy(
                "No active strategies processed the event".into(),
            ));
        }

        Ok(())
    }

    pub fn get_active_strategies(&self) -> Vec<StrategyType> {
        self.strategies
            .iter()
            .filter(|(_, strategy)| strategy.is_active())
            .map(|(strategy_type, _)| *strategy_type)
            .collect()
    }

    pub fn strategy_count(&self) -> usize {
        self.strategies.len()
    }

    pub fn active_strategy_count(&self) -> usize {
        self.strategies
            .values()
            .filter(|strategy| strategy.is_active())
            .count()
    }
}

// Placeholder strategy implementations - these would be replaced with actual strategy logic

pub struct MomentumStrategy {
    active: bool,
}

impl MomentumStrategy {
    pub fn new() -> Self {
        Self { active: true }
    }
}

#[async_trait]
impl StrategyTrait for MomentumStrategy {
    async fn process_event(&mut self, _event: &Event) -> Result<()> {
        // Placeholder - actual momentum analysis would go here
        Ok(())
    }

    fn get_type(&self) -> StrategyType {
        StrategyType::Momentum
    }

    fn is_active(&self) -> bool {
        self.active
    }
}

pub struct MeanReversionStrategy {
    active: bool,
}

impl MeanReversionStrategy {
    pub fn new() -> Self {
        Self { active: true }
    }
}

#[async_trait]
impl StrategyTrait for MeanReversionStrategy {
    async fn process_event(&mut self, _event: &Event) -> Result<()> {
        // Placeholder - actual mean reversion analysis would go here
        Ok(())
    }

    fn get_type(&self) -> StrategyType {
        StrategyType::MeanReversion
    }

    fn is_active(&self) -> bool {
        self.active
    }
}

pub struct ArbitrageStrategy {
    active: bool,
}

impl ArbitrageStrategy {
    pub fn new() -> Self {
        Self { active: true }
    }
}

#[async_trait]
impl StrategyTrait for ArbitrageStrategy {
    async fn process_event(&mut self, _event: &Event) -> Result<()> {
        // Placeholder - actual arbitrage detection would go here
        Ok(())
    }

    fn get_type(&self) -> StrategyType {
        StrategyType::Arbitrage
    }

    fn is_active(&self) -> bool {
        self.active
    }
}

pub fn initialize_strategies() -> StrategyRegistry {
    let mut registry = StrategyRegistry::new();

    // Use the default strategies from the strategies module
    let strategies = crate::strategies::default_strategies();
    for (_, strategy) in strategies {
        registry.register_strategy(strategy);
    }

    info!("Initialized {} strategies", registry.strategy_count());
    registry
}
