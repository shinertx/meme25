use crate::risk_management::circuit_breaker::CircuitBreaker;
use chrono::Utc;
use redis::AsyncCommands;
use reqwest::Client;
use shared_models::error::{ModelError, Result};
use std::time::Duration;
use tracing::{error, info, warn};

impl CircuitBreaker {
    pub async fn tick(self) -> Result<()> {
        let mut redis_conn = self
            .redis
            .get_async_connection()
            .await
            .map_err(|e| ModelError::Redis(e.to_string()))?;

        loop {
            // Check multiple risk metrics
            let drawdown: f64 = redis_conn.get("portfolio:drawdown").await.unwrap_or(0.0);

            let daily_loss: f64 = redis_conn.get("portfolio:daily_pnl").await.unwrap_or(0.0);

            let open_positions: i32 = redis_conn
                .get("portfolio:open_positions")
                .await
                .unwrap_or(0);

            // Circuit breaker conditions per Copilot instructions
            let mut halt_trading = false;
            let mut reason = String::new();

            if drawdown <= -0.10 {
                // 10% drawdown
                halt_trading = true;
                reason = format!("Drawdown limit breached: {:.2}%", drawdown * 100.0);
            }

            if daily_loss <= -50.0 {
                // $50 daily loss (25% of $200)
                halt_trading = true;
                reason = format!("Daily loss limit breached: ${:.2}", daily_loss);
            }

            if open_positions > 10 {
                // Max 10 concurrent positions
                halt_trading = true;
                reason = format!("Too many open positions: {}", open_positions);
            }

            if halt_trading {
                error!("🚨 CIRCUIT BREAKER TRIPPED: {}", reason);

                // Publish halt command
                let _: () = redis_conn
                    .publish("control:trading", "HALT")
                    .await
                    .map_err(|e| ModelError::Redis(e.to_string()))?;

                // Set system state
                let _: () = redis_conn
                    .set("system:trading_enabled", "false")
                    .await
                    .map_err(|e| ModelError::Redis(e.to_string()))?;

                // Send alert (in production, this would page on-call)
                self.send_alert(&reason).await?;
            }

            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }

    async fn send_alert(&self, reason: &str) -> Result<()> {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| {
                ModelError::Network(format!("Failed to build alert HTTP client: {}", e))
            })?;

        let mut delivered = false;

        if let Ok(webhook) = std::env::var("SLACK_WEBHOOK_URL") {
            let payload = serde_json::json!({
                "text": format!("🚨 MemeSnipe circuit breaker triggered: {}", reason),
            });

            match client.post(&webhook).json(&payload).send().await {
                Ok(resp) if resp.status().is_success() => {
                    info!("Sent Slack circuit breaker alert");
                    delivered = true;
                }
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    warn!("Failed to send Slack alert (status {}): {}", status, body);
                }
                Err(e) => warn!("Slack alert request failed: {}", e),
            }
        }

        if let Ok(routing_key) = std::env::var("PAGERDUTY_ROUTING_KEY") {
            let payload = serde_json::json!({
                "routing_key": routing_key,
                "event_action": "trigger",
                "dedup_key": format!("meme25-circuit-breaker-{}", chrono::Utc::now().timestamp()),
                "payload": {
                    "summary": format!("Circuit breaker triggered: {}", reason),
                    "source": "meme25.executor",
                    "severity": "critical",
                    "component": "risk_engine",
                    "group": "trading_system",
                    "class": "circuit_breaker"
                }
            });

            match client
                .post("https://events.pagerduty.com/v2/enqueue")
                .json(&payload)
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    info!("Sent PagerDuty incident for circuit breaker");
                    delivered = true;
                }
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    warn!(
                        "Failed to send PagerDuty incident (status {}): {}",
                        status, body
                    );
                }
                Err(e) => warn!("PagerDuty alert request failed: {}", e),
            }
        }

        if !delivered {
            warn!(
                "Circuit breaker triggered but no alert endpoints succeeded: {}",
                reason
            );
        }

        Ok(())
    }
}
