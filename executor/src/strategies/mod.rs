// Strategy modules
pub mod airdrop_rotation;
pub mod bridge_inflow;
pub mod dev_wallet_drain;
pub mod korean_time_burst;
pub mod liquidity_migration;
pub mod mean_revert_1h;
pub mod momentum_5m;
pub mod perp_basis_arb;
pub mod rug_pull_sniffer;
pub mod social_buzz;

// Re-export Strategy trait and types
use crate::strategy_registry::StrategyTrait;
use airdrop_rotation::AirdropRotation;
use async_trait::async_trait;
use bridge_inflow::BridgeInflow;
use dev_wallet_drain::DevWalletDrain;
use korean_time_burst::KoreanTimeBurst;
use liquidity_migration::LiquidityMigration;
use mean_revert_1h::MeanRevert1h;
use momentum_5m::Momentum5m;
use perp_basis_arb::PerpBasisArb;
use rug_pull_sniffer::RugPullSniffer;
use serde_json::{json, Value};
use shared_models::error::ModelError;
use shared_models::{Event, StrategyType};
pub use shared_models::{EventType, MarketEvent, OrderDetails, Strategy, StrategyAction};
use social_buzz::SocialBuzz;
use std::collections::HashMap;
use tracing::debug;

#[derive(Clone)]
pub struct LiveStrategyConfig {
    pub id: &'static str,
    pub params: Value,
    pub constructor: fn() -> Box<dyn Strategy + Send + Sync>,
    pub default_live: bool,
}

impl LiveStrategyConfig {
    pub fn instantiate(&self) -> Box<dyn Strategy + Send + Sync> {
        (self.constructor)()
    }
}

pub const DEFAULT_LIVE_STRATEGIES: &[&str] = &["momentum_5m", "mean_revert_1h"];

struct StrategyAdapter<T: Strategy + Send + Sync + 'static> {
    inner: T,
    strategy_type: StrategyType,
    active: bool,
    initialized: bool,
    params: Value,
}

impl<T: Strategy + Send + Sync + 'static> StrategyAdapter<T> {
    fn new(strategy_type: StrategyType, inner: T) -> Self {
        Self {
            inner,
            strategy_type,
            active: true,
            initialized: false,
            params: json!({}),
        }
    }

    fn with_params(mut self, params: Value) -> Self {
        self.params = params;
        self
    }

    fn boxed(strategy_type: StrategyType, inner: T) -> Box<dyn StrategyTrait> {
        Box::new(Self::new(strategy_type, inner))
    }

    fn boxed_with_params(
        strategy_type: StrategyType,
        inner: T,
        params: Value,
    ) -> Box<dyn StrategyTrait> {
        Box::new(Self::new(strategy_type, inner).with_params(params))
    }
}

#[async_trait]
impl<T> StrategyTrait for StrategyAdapter<T>
where
    T: Strategy + Send + Sync + 'static,
{
    async fn process_event(&mut self, event: &Event) -> shared_models::error::Result<()> {
        if !self.active {
            return Ok(());
        }

        // Lazy one-time initialization with params
        if !self.initialized {
            self.inner.init(&self.params).await.map_err(|e| {
                ModelError::Strategy(format!("{} init failed: {}", self.inner.id(), e))
            })?;
            self.initialized = true;
        }

        let market_event = match event {
            Event::Market(ref market_event) => market_event,
        };

        let action = self
            .inner
            .on_event(market_event)
            .await
            .map_err(|e| ModelError::Strategy(format!("{} failed: {}", self.inner.id(), e)))?;

        if !matches!(action, StrategyAction::Hold) {
            debug!(
                strategy = self.inner.id(),
                ?action,
                "Strategy generated action"
            );
        }

        Ok(())
    }

    fn get_type(&self) -> StrategyType {
        self.strategy_type
    }

    fn is_active(&self) -> bool {
        self.active
    }
}

// RC3 Addendum: Strategy instantiation helper (Closes #6)
pub fn default_strategies() -> HashMap<String, Box<dyn StrategyTrait>> {
    let mut strategies: HashMap<String, Box<dyn StrategyTrait>> = HashMap::new();

    // Paper-ready defaults
    // More conservative momentum defaults in weak market regimes:
    // - longer lookback and higher volume multiple reduce false positives
    // - higher price change threshold requires stronger momentum bursts
    // - regime SMA + cooldown to avoid chop
    // - enforce high-liquidity tokens only ($10M+)
    let momentum_params = json!({
        "lookback": 36,
        "vol_multiplier": 2.5,
        "price_change_threshold": 0.02,
        "regime_sma_lookback": 24,
        "cooldown_minutes": 20,
        "min_liquidity_usd": 10_000_000.0
    });
    strategies.insert(
        "momentum_5m".into(),
        StrategyAdapter::boxed_with_params(
            StrategyType::Momentum,
            Momentum5m::default(),
            momentum_params,
        ),
    );

    let mean_rev_params = json!({
        "period_hours": 6,
        "z_score_threshold": 1.8
    });
    strategies.insert(
        "mean_revert_1h".into(),
        StrategyAdapter::boxed_with_params(
            StrategyType::MeanReversion,
            MeanRevert1h::default(),
            mean_rev_params,
        ),
    );

    strategies.insert(
        "social_buzz".into(),
        StrategyAdapter::boxed(StrategyType::SocialSentiment, SocialBuzz::default()),
    );
    strategies.insert(
        "liquidity_migration".into(),
        StrategyAdapter::boxed(StrategyType::LiquidityMining, LiquidityMigration::default()),
    );
    strategies.insert(
        "perp_basis_arb".into(),
        StrategyAdapter::boxed(StrategyType::Arbitrage, PerpBasisArb::default()),
    );
    strategies.insert(
        "dev_wallet_drain".into(),
        StrategyAdapter::boxed(StrategyType::EventDriven, DevWalletDrain::default()),
    );
    strategies.insert(
        "airdrop_rotation".into(),
        StrategyAdapter::boxed(StrategyType::TrendFollowing, AirdropRotation::default()),
    );
    strategies.insert(
        "korean_time_burst".into(),
        StrategyAdapter::boxed(StrategyType::BreakoutReversion, KoreanTimeBurst::default()),
    );
    strategies.insert(
        "bridge_inflow".into(),
        StrategyAdapter::boxed(StrategyType::CrossChainArb, BridgeInflow::default()),
    );
    strategies.insert(
        "rug_pull_sniffer".into(),
        StrategyAdapter::boxed(StrategyType::VolumeAnomaly, RugPullSniffer::default()),
    );

    strategies
}

pub fn live_strategy_configs() -> Vec<LiveStrategyConfig> {
    vec![
        LiveStrategyConfig {
            id: "momentum_5m",
            params: json!({
                // Mirror conservative defaults used for paper trading
                "lookback": 32,
                "vol_multiplier": 2.0,
                "price_change_threshold": 0.018,
                "regime_sma_lookback": 24,
                "cooldown_minutes": 18,
                "min_liquidity_usd": 10_000_000.0,
                "allowed_tokens": [
                    "DezXAZ8z7PnrnRJjz3E2YkdY8YPh91qCP83N5dEJ9h5z" // BONK
                ]
            }),
            constructor: || Box::new(Momentum5m::default()),
            default_live: true,
        },
        LiveStrategyConfig {
            id: "mean_revert_1h",
            params: json!({
                "period_hours": 6,
                "z_score_threshold": 1.8
            }),
            constructor: || Box::new(MeanRevert1h::default()),
            default_live: true,
        },
        LiveStrategyConfig {
            id: "social_buzz",
            params: json!({}),
            constructor: || Box::new(SocialBuzz::default()),
            default_live: false,
        },
        LiveStrategyConfig {
            id: "liquidity_migration",
            params: json!({}),
            constructor: || Box::new(LiquidityMigration::default()),
            default_live: false,
        },
        LiveStrategyConfig {
            id: "perp_basis_arb",
            params: json!({}),
            constructor: || Box::new(PerpBasisArb::default()),
            default_live: false,
        },
        LiveStrategyConfig {
            id: "dev_wallet_drain",
            params: json!({}),
            constructor: || Box::new(DevWalletDrain::default()),
            default_live: false,
        },
        LiveStrategyConfig {
            id: "airdrop_rotation",
            params: json!({}),
            constructor: || Box::new(AirdropRotation::default()),
            default_live: false,
        },
        LiveStrategyConfig {
            id: "korean_time_burst",
            params: json!({}),
            constructor: || Box::new(KoreanTimeBurst::default()),
            default_live: false,
        },
        LiveStrategyConfig {
            id: "bridge_inflow",
            params: json!({}),
            constructor: || Box::new(BridgeInflow::default()),
            default_live: false,
        },
        LiveStrategyConfig {
            id: "rug_pull_sniffer",
            params: json!({}),
            constructor: || Box::new(RugPullSniffer::default()),
            default_live: false,
        },
    ]
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
