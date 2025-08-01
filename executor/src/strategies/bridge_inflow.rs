use crate::strategies::{Strategy, MarketEvent, StrategyAction, EventType};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BridgeInflow;

#[async_trait]
impl Strategy for BridgeInflow {
    fn id(&self) -> &'static str { "bridge_inflow" }
    fn subscriptions(&self) -> HashSet<EventType> { [EventType::Bridge].iter().cloned().collect() }
    async fn init(&mut self, _params: &Value) -> Result<()> { Ok(()) }
    async fn on_event(&mut self, _event: &MarketEvent) -> Result<StrategyAction> { Ok(StrategyAction::Hold) }
    fn get_state(&self) -> Value { serde_json::json!({}) }
}
