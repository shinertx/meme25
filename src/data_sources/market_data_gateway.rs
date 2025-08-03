use crate::prelude::*;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::Message};

async fn connect_helius(&self, url: &str) -> Result<()> {
        info!("Connecting to Helius WebSocket with auth...");
        
        let url = url::Url::parse(url)
            .context("Failed to parse Helius URL")?;
        
        let (mut ws_stream, _) = connect_async(url).await
            .context("Failed to connect to Helius WebSocket")?;
        
        info!("Connected to Helius WebSocket successfully");
        
        // Subscribe to all relevant streams
        let subscribe_msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "blockSubscribe",
            "params": {
                "filter": "all"
            }
        });
        
        ws_stream.send(Message::Text(subscribe_msg.to_string())).await
            .context("Failed to send subscription")?;
        
        Ok(())
    }