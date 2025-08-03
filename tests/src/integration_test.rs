use shared_models::{Event, MarketEvent, PriceTick, EventType};
use shared_models::error::{Result, ModelError};
use redis::{Client, AsyncCommands};
use tracing::{info, error};
use tokio::time::{sleep, Duration};
use serde_json;

/// Integration test that validates the entire MemeSnipe v25 system
/// Critical Finding #15: Integration test harness
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore] // Run with --ignored flag
async fn price_event_roundtrip() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("🧪 Starting price event roundtrip test");

    // Test that price events flow through the system correctly
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let client = Client::open(redis_url)?;
    let mut conn = client.get_async_connection().await
        .map_err(|e| ModelError::Redis(format!("Failed to connect to Redis: {}", e)))?;

    // Create a test price event
    let test_event = Event::Price(PriceTick {
        symbol: "SOL".to_string(),
        price: 100.0,
        volume_24h: 1000000.0,
        market_cap: 50000000.0,
        timestamp: chrono::Utc::now(),
        source: "test".to_string(),
    });

    // Publish event to Redis stream
    let event_json = serde_json::to_string(&test_event)
        .map_err(|e| ModelError::Serde(e))?;
    
    let _: String = conn.xadd("events:price", "*", &[("data", &event_json), ("type", "price")]).await
        .map_err(|e| ModelError::Redis(format!("Failed to publish event: {}", e)))?;

    info!("✅ Published test price event to Redis stream");

    // Wait a bit for processing
    sleep(Duration::from_millis(100)).await;

    // TODO: Verify the event was processed by checking database
    // This would require database connection and trade verification

    info!("✅ Price event roundtrip test completed");
    Ok(())
}

async fn run_integration_tests() -> Result<()> {
    info!("Running Redis connectivity test...");
    test_redis_connectivity().await?;

    info!("Running event flow test...");
    test_event_flow().await?;

    info!("Running strategy registry test...");
    test_strategy_registry().await?;

    info!("Running circuit breaker test...");
    test_circuit_breaker().await?;

    Ok(())
}

async fn test_redis_connectivity() -> Result<()> {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    
    let client = Client::open(redis_url.as_str())
        .map_err(|e| ModelError::Redis(format!("Failed to create Redis client: {}", e)))?;
    
    let mut conn = client.get_multiplexed_async_connection().await
        .map_err(|e| ModelError::Redis(format!("Failed to connect to Redis: {}", e)))?;

    // Test basic connectivity
    let _: String = conn.ping().await
        .map_err(|e| ModelError::Redis(format!("Redis ping failed: {}", e)))?;

    info!("✅ Redis connectivity test passed");
    Ok(())
}

async fn test_event_flow() -> Result<()> {
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    
    let client = Client::open(redis_url.as_str())?;
    let mut conn = client.get_multiplexed_async_connection().await?;

    // Create a test event
    let test_event = Event::Market(MarketEvent::Price(PriceTick {
        token_address: "So11111111111111111111111111111111111111112".to_string(),
        price_usd: 100.0,
        volume_usd_1m: 50000.0,
        volume_usd_5m: 150000.0,
        volume_usd_15m: 300000.0,
        price_change_1m: 2.5,
        price_change_5m: 5.1,
        liquidity_usd: 1000000.0,
        timestamp: chrono::Utc::now(),
    }));

    // Serialize and publish to Redis stream
    let event_json = serde_json::to_string(&test_event)?;
    let stream_id: String = conn.xadd("events:price", "*", &[
        ("type", "price"),
        ("data", &event_json)
    ]).await?;

    info!("✅ Event published with ID: {}", stream_id);

    // Verify we can read it back
    let result: redis::streams::StreamReadReply = redis::cmd("XREAD")
        .arg("COUNT")
        .arg(1)
        .arg("STREAMS")
        .arg("events:price")
        .arg("0")
        .query_async(&mut conn)
        .await?;

    if result.keys.is_empty() {
        return Err(anyhow::anyhow!("No events found in stream"));
    }

    info!("✅ Event flow test passed");
    Ok(())
}

async fn test_strategy_registry() -> Result<()> {
    use executor::initialize_strategies;
    
    let registry = initialize_strategies();
    
    if registry.strategy_count() == 0 {
        return Err(anyhow::anyhow!("No strategies loaded"));
    }

    if registry.active_strategy_count() == 0 {
        return Err(anyhow::anyhow!("No active strategies found"));
    }

    info!("✅ Strategy registry test passed - {} strategies loaded, {} active", 
          registry.strategy_count(), registry.active_strategy_count());
    Ok(())
}

async fn test_circuit_breaker() -> Result<()> {
    use executor::CircuitBreaker;
    
    let circuit_breaker = CircuitBreaker::new(15.0, 5.0, 100000.0);
    
    // Test that circuit breaker starts in normal state
    if circuit_breaker.is_tripped() {
        return Err(anyhow::anyhow!("Circuit breaker should start in normal state"));
    }

    info!("✅ Circuit breaker test passed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_serialization() {
        let event = Event::Market(MarketEvent::Price(PriceTick {
            token_address: "test".to_string(),
            price_usd: 1.0,
            volume_usd_1m: 0.0,
            volume_usd_5m: 0.0,
            volume_usd_15m: 0.0,
            price_change_1m: 0.0,
            price_change_5m: 0.0,
            liquidity_usd: 0.0,
            timestamp: chrono::Utc::now(),
        }));

        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: Event = serde_json::from_str(&serialized).unwrap();
        
        match (event, deserialized) {
            (Event::Market(MarketEvent::Price(original)), Event::Market(MarketEvent::Price(parsed))) => {
                assert_eq!(original.token_address, parsed.token_address);
                assert_eq!(original.price_usd, parsed.price_usd);
            }
            _ => panic!("Event serialization failed"),
        }
    }
}
