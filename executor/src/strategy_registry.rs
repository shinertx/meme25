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
    strategies: HashMap<String, Box<dyn StrategyTrait>>,
}

impl StrategyRegistry {
    pub fn new() -> Self {
        Self {
            strategies: HashMap::new(),
        }
    }

    pub fn register_strategy(&mut self, id: String, strategy: Box<dyn StrategyTrait>) {
        info!("Registering strategy: {} ({:?})", id, strategy.get_type());
        self.strategies.insert(id, strategy);
    }

    pub async fn process_event(&mut self, event: &Event) -> Result<()> {
        let mut processed_count = 0;

        for strategy in self.strategies.values_mut() {
            if strategy.is_active() {
                match strategy.process_event(event).await {
                    Ok(()) => {
                        processed_count += 1;
                    }
                    Err(e) => {
                        error!(
                            "Strategy {:?} failed to process event: {}",
                            strategy.get_type(),
                            e
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
        let mut active: Vec<StrategyType> = Vec::new();
        for strategy in self.strategies.values() {
            if strategy.is_active() {
                let strategy_type = strategy.get_type();
                if !active.contains(&strategy_type) {
                    active.push(strategy_type);
                }
            }
        }
        active
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
    for (id, strategy) in strategies {
        registry.register_strategy(id, strategy);
    }

    info!("Initialized {} strategies", registry.strategy_count());
    registry
}
