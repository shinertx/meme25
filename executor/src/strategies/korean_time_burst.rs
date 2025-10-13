use crate::strategies::{EventType, MarketEvent, Strategy, StrategyAction};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct KoreanTimeBurst;

#[async_trait]
impl Strategy for KoreanTimeBurst {
    fn id(&self) -> &'static str {
        "korean_time_burst"
    }
    fn subscriptions(&self) -> HashSet<EventType> {
        [EventType::Volume].iter().cloned().collect()
    }
    async fn init(&mut self, _params: &Value) -> Result<()> {
        Ok(())
    }
    async fn on_event(&mut self, _event: &MarketEvent) -> Result<StrategyAction> {
        Ok(StrategyAction::Hold)
    }
    fn get_state(&self) -> Value {
        serde_json::json!({})
    }
}
