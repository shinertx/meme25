use crate::strategies::{Strategy, MarketEvent, StrategyAction, EventType};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RugPullSniffer;

#[async_trait]
impl Strategy for RugPullSniffer {
    fn id(&self) -> &'static str { "rug_pull_sniffer" }
    fn subscriptions(&self) -> HashSet<EventType> { [EventType::OnChain, EventType::Whale].iter().cloned().collect() }
    async fn init(&mut self, _params: &Value) -> Result<()> { Ok(()) }
    async fn on_event(&mut self, _event: &MarketEvent) -> Result<StrategyAction> { Ok(StrategyAction::Hold) }
    fn get_state(&self) -> Value { serde_json::json!({}) }
}
