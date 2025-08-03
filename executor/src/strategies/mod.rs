// Strategy modules
pub mod momentum_5m;
pub mod mean_revert_1h;
pub mod social_buzz;
pub mod liquidity_migration;
pub mod perp_basis_arb;
pub mod dev_wallet_drain;
pub mod airdrop_rotation;
pub mod korean_time_burst;
pub mod bridge_inflow;
pub mod rug_pull_sniffer;

// Re-export Strategy trait and types
pub use shared_models::{Strategy, MarketEvent, StrategyAction, OrderDetails, EventType};
use crate::strategy_registry::{StrategyTrait, StrategyRegistry};
use std::collections::HashMap;

// RC3 Addendum: Strategy instantiation helper (Closes #6)
pub fn default_strategies() -> HashMap<String, Box<dyn StrategyTrait>> {
    let mut strategies: HashMap<String, Box<dyn StrategyTrait>> = HashMap::new();
    
    // Note: These would be actual strategy implementations
    // For now, using placeholder strategies from strategy_registry
    strategies.insert("momentum_5m".into(), Box::new(crate::strategy_registry::MomentumStrategy::new()));
    strategies.insert("mean_reversion_1h".into(), Box::new(crate::strategy_registry::MeanReversionStrategy::new()));
    strategies.insert("breakout".into(), Box::new(crate::strategy_registry::ArbitrageStrategy::new()));
    
    // Add more strategies as they are implemented
    // strategies.insert("cross_chain_arb".into(), Box::new(CrossChainArbStrategy::new()));
    // strategies.insert("volume_spike".into(), Box::new(VolumeSpikeStrategy::new()));
    // strategies.insert("social_buzz".into(), Box::new(SocialBuzzStrategy::new()));
    // strategies.insert("whale_watch".into(), Box::new(WhaleWatchStrategy::new()));
    // strategies.insert("multi_signal".into(), Box::new(MultiSignalStrategy::new()));
    // strategies.insert("degen_mode".into(), Box::new(DegenModeStrategy::new()));
    // strategies.insert("mean_reversion_daily".into(), Box::new(MeanReversionDailyStrategy::new()));
    
    strategies
}

// Strategy registration helper
#[macro_export]
macro_rules! register_strategy {
    ($strategy_type:ty, $strategy_id:expr) => {
        inventory::submit! {
            $crate::strategies::StrategyRegistration {
                id: $strategy_id,
                constructor: || Box::new(<$strategy_type>::default()),
            }
        }
    };
}

pub struct StrategyRegistration {
    pub id: &'static str,
    pub constructor: fn() -> Box<dyn Strategy>,
}

inventory::collect!(StrategyRegistration);
