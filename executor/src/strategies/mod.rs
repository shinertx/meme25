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
