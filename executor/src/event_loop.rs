use crate::strategy_registry::StrategyRegistry;
use redis::{Client, RedisResult};
use shared_models::error::{ModelError, Result};
use shared_models::Event;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

pub struct EventLoop {
    redis_client: Client,
    strategy_registry: StrategyRegistry,
    consumer_group: String,
    consumer_name: String,
    stream_keys: Vec<String>,
}

impl EventLoop {
    pub fn new(redis_url: &str, strategy_registry: StrategyRegistry) -> Result<Self> {
        let redis_client = Client::open(redis_url)
            .map_err(|e| ModelError::Redis(format!("Failed to create Redis client: {}", e)))?;

        Ok(EventLoop {
            redis_client,
            strategy_registry,
            consumer_group: "executor_group".to_string(),
            consumer_name: "executor_1".to_string(),
            stream_keys: vec![
                "events:price".to_string(),
                "events:social".to_string(),
                "events:depth".to_string(),
                "events:bridge".to_string(),
                "events:funding".to_string(),
                "events:onchain".to_string(),
                "events:solprice".to_string(),
                "events:twitter".to_string(),
                "events:farcaster".to_string(),
                "events:whale".to_string(),
                "events:liquidation".to_string(),
            ],
        })
    }

    pub async fn initialize(&self) -> Result<()> {
        let mut conn = self
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| ModelError::Redis(format!("Failed to get Redis connection: {}", e)))?;

        // Create consumer groups for all stream keys
        for stream_key in &self.stream_keys {
            let result: RedisResult<String> = redis::cmd("XGROUP")
                .arg("CREATE")
                .arg(stream_key)
                .arg(&self.consumer_group)
                .arg("$")
                .arg("MKSTREAM")
                .query_async(&mut conn)
                .await;

            match result {
                Ok(_) => {
                    info!(
                        "Created consumer group '{}' for stream '{}'",
                        self.consumer_group, stream_key
                    );
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    if error_msg.contains("BUSYGROUP") {
                        info!(
                            "Consumer group '{}' already exists for stream '{}'",
                            self.consumer_group, stream_key
                        );
                    } else {
                        warn!(
                            "Failed to create consumer group '{}' for stream '{}': {}",
                            self.consumer_group, stream_key, e
                        );
                        // Continue with other streams instead of failing entirely
                    }
                }
            }
        }

        info!(
            "Event loop initialized with {} stream keys",
            self.stream_keys.len()
        );
        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("Starting event loop");
        let mut conn = self
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| ModelError::Redis(format!("Failed to get Redis connection: {}", e)))?;

        let active_strategies = self.strategy_registry.get_active_strategies();
        info!(
            "Event loop running with {} active strategies: {:?}",
            active_strategies.len(),
            active_strategies
        );

        loop {
            match self.read_events(&mut conn).await {
                Ok(events_processed) => {
                    if events_processed > 0 {
                        debug!("Processed {} events this cycle", events_processed);
                    }
                }
                Err(e) => {
                    error!("Error in event loop: {}", e);
                    sleep(Duration::from_secs(1)).await;
                }
            }

            // Small delay to prevent overwhelming the system
            sleep(Duration::from_millis(10)).await;
        }
    }

    async fn read_events(&mut self, conn: &mut redis::aio::MultiplexedConnection) -> Result<u32> {
        let mut events_processed = 0;

        // Read from all streams
        for stream_key in &self.stream_keys.clone() {
            match self.read_stream_events(conn, stream_key).await {
                Ok(count) => events_processed += count,
                Err(e) => {
                    // Be less verbose about Redis connection issues to prevent log spam
                    if e.to_string().contains("NOGROUP") {
                        debug!("Consumer group issue for stream {}: {}", stream_key, e);
                    } else {
                        warn!("Failed to read from stream {}: {}", stream_key, e);
                    }
                }
            }
        }

        Ok(events_processed)
    }

    async fn read_stream_events(
        &mut self,
        conn: &mut redis::aio::MultiplexedConnection,
        stream_key: &str,
    ) -> Result<u32> {
        let result: RedisResult<redis::streams::StreamReadReply> = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(&self.consumer_group)
            .arg(&self.consumer_name)
            .arg("COUNT")
            .arg(10)
            .arg("BLOCK")
            .arg(100)
            .arg("STREAMS")
            .arg(stream_key)
            .arg(">")
            .query_async(conn)
            .await;

        let reply = match result {
            Ok(reply) => reply,
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("NOGROUP") {
                    // Consumer group doesn't exist, try to create it
                    warn!(
                        "Consumer group '{}' not found for stream '{}', attempting to create it",
                        self.consumer_group, stream_key
                    );

                    let create_result: RedisResult<String> = redis::cmd("XGROUP")
                        .arg("CREATE")
                        .arg(stream_key)
                        .arg(&self.consumer_group)
                        .arg("$")
                        .arg("MKSTREAM")
                        .query_async(conn)
                        .await;

                    match create_result {
                        Ok(_) => {
                            info!(
                                "Successfully created consumer group '{}' for stream '{}'",
                                self.consumer_group, stream_key
                            );
                            return Ok(0); // Return 0 events processed, will try again next cycle
                        }
                        Err(create_err) => {
                            if create_err.to_string().contains("BUSYGROUP") {
                                info!(
                                    "Consumer group '{}' already exists for stream '{}'",
                                    self.consumer_group, stream_key
                                );
                                return Ok(0);
                            } else {
                                return Err(ModelError::Redis(format!(
                                    "Failed to create consumer group for stream {}: {}",
                                    stream_key, create_err
                                )));
                            }
                        }
                    }
                } else {
                    return Err(ModelError::Redis(format!(
                        "Failed to read from stream {}: {}",
                        stream_key, e
                    )));
                }
            }
        };

        let mut events_processed = 0;

        for stream in reply.keys {
            for stream_id in stream.ids {
                match self.parse_and_process_event(&stream_id.map).await {
                    Ok(()) => {
                        events_processed += 1;
                        // Acknowledge the message
                        let _: RedisResult<u32> = redis::cmd("XACK")
                            .arg(stream_key)
                            .arg(&self.consumer_group)
                            .arg(&stream_id.id)
                            .query_async(conn)
                            .await;
                    }
                    Err(e) => {
                        error!("Failed to process event from stream {}: {}", stream_key, e);
                    }
                }
            }
        }

        Ok(events_processed)
    }

    async fn parse_and_process_event(
        &mut self,
        event_data: &std::collections::HashMap<String, redis::Value>,
    ) -> Result<()> {
        // Extract event type and data
        let event_type_value = event_data
            .get("type")
            .ok_or_else(|| ModelError::Redis("Missing event type".into()))?;

        let event_data_value = event_data
            .get("data")
            .ok_or_else(|| ModelError::Redis("Missing event data".into()))?;

        let event_type_str = match event_type_value {
            redis::Value::Data(bytes) => std::str::from_utf8(bytes)
                .map_err(|e| ModelError::Redis(format!("Invalid UTF-8 in event type: {}", e)))?,
            _ => return Err(ModelError::Redis("Event type is not a string".into())),
        };

        let event_data_str = match event_data_value {
            redis::Value::Data(bytes) => std::str::from_utf8(bytes)
                .map_err(|e| ModelError::Redis(format!("Invalid UTF-8 in event data: {}", e)))?,
            _ => return Err(ModelError::Redis("Event data is not a string".into())),
        };

        // Parse the event data JSON
        let event: Event = serde_json::from_str(event_data_str).map_err(ModelError::Serde)?;

        debug!("Processing event type: {}", event_type_str);

        // Send event to all active strategies
        self.strategy_registry.process_event(&event).await?;

        Ok(())
    }

    pub fn get_active_strategy_count(&self) -> usize {
        self.strategy_registry.active_strategy_count()
    }
}
