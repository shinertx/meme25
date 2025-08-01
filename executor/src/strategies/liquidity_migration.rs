use crate::strategies::{Strategy, MarketEvent, StrategyAction, EventType};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct LiquidityMigration;

#[async_trait]
impl Strategy for LiquidityMigration {
    fn id(&self) -> &'static str { "liquidity_migration" }
    fn subscriptions(&self) -> HashSet<EventType> { [EventType::OnChain].iter().cloned().collect() }
    async fn init(&mut self, _params: &Value) -> Result<()> { Ok(()) }
    async fn on_event(&mut self, _event: &MarketEvent) -> Result<StrategyAction> { Ok(StrategyAction::Hold) }
    fn get_state(&self) -> Value { serde_json::json!({}) }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PerpBasisArb;

#[async_trait]
impl Strategy for PerpBasisArb {
    fn id(&self) -> &'static str { "perp_basis_arb" }
    fn subscriptions(&self) -> HashSet<EventType> { [EventType::Funding].iter().cloned().collect() }
    async fn init(&mut self, _params: &Value) -> Result<()> { Ok(()) }
    async fn on_event(&mut self, _event: &MarketEvent) -> Result<StrategyAction> { Ok(StrategyAction::Hold) }
    fn get_state(&self) -> Value { serde_json::json!({}) }
}
