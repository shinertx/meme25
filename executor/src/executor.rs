use crate::config;
use crate::database::Database;
use crate::jupiter::{JupiterClient, QuoteRequest, SwapRequest};
use crate::mev_protection::{JitoClient as MevJitoClient, MevProtectionManager};
use crate::risk_manager::{RiskManager, TradeDecision};
use crate::signer_client;
use crate::strategies::{live_strategy_configs, Strategy, DEFAULT_LIVE_STRATEGIES};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use redis::{
    self, aio::MultiplexedConnection, streams::StreamReadReply, Client as RedisClient, Value,
};
use reqwest::Client as HttpClient;
use serde_json::json;
use serde_json::Value as JsonValue;
use shared_models::{
    Event, EventType, MarketEvent, OrderDetails, RiskEvent, StrategyAction, Trade,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tracing::{debug, info, warn};
use uuid::Uuid;

struct StrategySlot {
    id: String,
    params: JsonValue,
    strategy: Box<dyn Strategy + Send + Sync>,
    initialized: bool,
    subscriptions: HashSet<EventType>,
}

pub struct TradingExecutor {
    strategies: HashMap<String, StrategySlot>,
    active_positions: Arc<RwLock<HashMap<String, f64>>>,
    paper_trading: bool,
    allowed_live_strategies: Option<HashSet<String>>,
}

impl TradingExecutor {
    pub fn new(paper_trading: bool, allowed_live_strategies: Option<HashSet<String>>) -> Self {
        Self {
            strategies: HashMap::new(),
            active_positions: Arc::new(RwLock::new(HashMap::new())),
            paper_trading,
            allowed_live_strategies,
        }
    }

    pub fn add_strategy(
        &mut self,
        name: String,
        strategy: Box<dyn Strategy + Send + Sync>,
        params: JsonValue,
    ) {
        let subscriptions = strategy.subscriptions();
        let slot = StrategySlot {
            id: name.clone(),
            params,
            strategy,
            initialized: false,
            subscriptions,
        };
        self.strategies.insert(name, slot);
    }

    pub fn strategy_ids(&self) -> Vec<String> {
        self.strategies.keys().cloned().collect()
    }

    pub async fn process_event(
        &mut self,
        event: &MarketEvent,
    ) -> Result<Vec<(String, StrategyAction)>> {
        let mut actions = Vec::new();
        let event_type = event.get_type();

        for (strategy_name, slot) in &mut self.strategies {
            if !slot.subscriptions.contains(&event_type) {
                continue;
            }

            if !self.paper_trading {
                if let Some(allow) = &self.allowed_live_strategies {
                    if !allow.contains(strategy_name) {
                        continue;
                    }
                }
            }

            if !slot.initialized {
                slot.strategy
                    .init(&slot.params)
                    .await
                    .with_context(|| format!("{} init failed", slot.id))?;
                slot.initialized = true;
            }

            match slot.strategy.on_event(event).await {
                Ok(action) => {
                    if !matches!(action, StrategyAction::Hold) {
                        tracing::info!(strategy = %strategy_name, ?action, "Strategy generated action");
                        actions.push((strategy_name.clone(), action));
                    }
                }
                Err(e) => {
                    tracing::error!("Strategy {} error: {}", strategy_name, e);
                }
            }
        }

        Ok(actions)
    }

    pub async fn execute_action(&self, action: StrategyAction) -> Result<()> {
        match action {
            StrategyAction::Execute(details) => {
                let side = match details.side {
                    shared_models::Side::Long => "BUY",
                    shared_models::Side::Short => "SELL",
                };

                if self.paper_trading {
                    self.paper_trade(side, &details).await?;
                } else {
                    self.live_trade(side, &details).await?;
                }
            }
            StrategyAction::Hold => {
                // No action needed
            }
            StrategyAction::ReducePosition(percentage) => {
                let reduction = percentage.clamp(0.0, 1.0);
                let (adjusted, closed) = self.reduce_positions(reduction).await;
                tracing::info!(
                    reduction_pct = reduction * 100.0,
                    positions_rebalanced = adjusted,
                    positions_closed = closed,
                    "Applied portfolio reduction request"
                );
            }
            StrategyAction::ClosePosition => {
                let closed = self.close_all_positions().await;
                tracing::info!(
                    positions_closed = closed,
                    "Closed all tracked positions on risk directive"
                );
            }
        }
        Ok(())
    }

    async fn paper_trade(&self, side: &str, details: &OrderDetails) -> Result<()> {
        tracing::info!(
            "📝 PAPER TRADE: {} {} USD in {} at confidence {}",
            side,
            details.suggested_size_usd,
            details.symbol,
            details.confidence
        );

        self.apply_position_change(side, details).await
    }

    async fn live_trade(&self, side: &str, details: &OrderDetails) -> Result<()> {
        info!(
            "🚀 LIVE TRADE: {} {} USD in {} (confidence {:.2})",
            side, details.suggested_size_usd, details.symbol, details.confidence
        );

        let config =
            config::get_config().map_err(|e| anyhow!("Configuration unavailable: {}", e))?;

        let api_url = std::env::var("JUPITER_BASE_URL")
            .unwrap_or_else(|_| "https://quote-api.jup.ag/v6".to_string());
        let jupiter = JupiterClient::new(api_url);

        let usdc_mint = std::env::var("USDC_MINT")
            .unwrap_or_else(|_| "EPjFWdd5AufqSSqeM2qZ8d8bP8XPhYet6GtDq3z31g".to_string());

        let (input_mint, output_mint, amount) = match side {
            "BUY" => {
                let amount = ((details.suggested_size_usd * 1_000_000.0).round() as u64).max(1);
                (usdc_mint.clone(), details.token_address.clone(), amount)
            }
            "SELL" => {
                let quantity_tokens = details
                    .strategy_metadata
                    .get("position_size_tokens")
                    .and_then(|v| v.as_f64())
                    .context(
                        "Missing position_size_tokens in strategy metadata for sell execution",
                    )?;
                let decimals = details
                    .strategy_metadata
                    .get("token_decimals")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(6);
                let scale = 10_f64.powi(decimals as i32);
                let amount = (quantity_tokens * scale).round() as u64;
                (
                    details.token_address.clone(),
                    usdc_mint.clone(),
                    amount.max(1),
                )
            }
            _ => return Err(anyhow!("Unsupported trade side: {}", side)),
        };

        let slippage_bps = details.risk_metrics.max_slippage_bps.max(25);
        let quote = jupiter
            .get_quote(QuoteRequest {
                input_mint: input_mint.clone(),
                output_mint: output_mint.clone(),
                amount,
                slippage_bps,
            })
            .await
            .context("Failed to fetch Jupiter quote")?;

        if quote.route_plan.is_empty() {
            warn!(
                symbol = %details.symbol,
                "Received Jupiter quote with empty route plan"
            );
        }

        let user_pubkey = signer_client::get_pubkey()
            .await
            .context("Failed to obtain signing pubkey")?;

        let swap = jupiter
            .get_swap(SwapRequest {
                quote: quote.clone(),
                user_public_key: user_pubkey.clone(),
                wrap_and_unwrap_sol: true,
            })
            .await
            .context("Failed to create Jupiter swap transaction")?;

        let signed_tx = signer_client::sign_transaction(&swap.swap_transaction)
            .await
            .context("Failed to sign transaction")?;

        let rpc_url = std::env::var("SOLANA_RPC_URL")
            .context("SOLANA_RPC_URL not set for trade submission")?;

        let jito_client = MevJitoClient::new(
            std::env::var("JITO_BLOCK_ENGINE_URL")
                .unwrap_or_else(|_| "https://mainnet.block-engine.jito.wtf".to_string()),
            std::env::var("JITO_AUTH_KEYPAIR").ok(),
            config.jito_tip_lamports,
        )
        .context("Failed to create Jito client")?;

        let mev_manager = MevProtectionManager::new(jito_client, config.jito_tip_lamports);
        let protection_level =
            mev_manager.determine_protection_level(details.suggested_size_usd, false, 0.05);

        let submission_result = mev_manager
            .submit_with_protection(signed_tx.clone(), protection_level)
            .await;

        let submission_reference = match submission_result {
            Ok(bundle_id) => {
                info!(
                    %bundle_id,
                    "Submitted live trade bundle via Jito for {} {}",
                    side,
                    details.symbol
                );
                bundle_id
            }
            Err(err) => {
                warn!(
                    error = %err,
                    "Jito submission failed, attempting direct RPC submission"
                );
                let signature = self
                    .submit_transaction_via_rpc(&rpc_url, &signed_tx)
                    .await
                    .context("Failed to submit transaction via fallback RPC path")?;
                info!(
                    %signature,
                    "Fallback RPC submission succeeded for {} {}",
                    side,
                    details.symbol
                );
                signature
            }
        };

        debug!(reference = %submission_reference, "Trade submission reference recorded");

        self.apply_position_change(side, details).await?;
        Ok(())
    }

    async fn apply_position_change(&self, side: &str, details: &OrderDetails) -> Result<()> {
        let mut positions = self.active_positions.write().await;
        let current_position = positions.get(&details.symbol).copied().unwrap_or(0.0);

        let quantity_change = match side {
            "BUY" => details.suggested_size_usd,
            "SELL" => -details.suggested_size_usd,
            _ => 0.0,
        };

        let new_position = current_position + quantity_change;

        if new_position.abs() < 0.01 {
            positions.remove(&details.symbol);
        } else {
            positions.insert(details.symbol.clone(), new_position);
        }

        Ok(())
    }

    async fn reduce_positions(&self, reduction: f64) -> (usize, usize) {
        let mut positions = self.active_positions.write().await;
        let mut removed = Vec::new();
        let mut adjusted = 0;

        for (symbol, value) in positions.iter_mut() {
            let new_size = *value * (1.0 - reduction);
            if new_size.abs() < 0.01 {
                removed.push(symbol.clone());
            } else {
                *value = new_size;
                adjusted += 1;
            }
        }

        for symbol in removed.iter() {
            positions.remove(symbol);
        }

        (adjusted, removed.len())
    }

    async fn close_all_positions(&self) -> usize {
        let mut positions = self.active_positions.write().await;
        let count = positions.len();
        positions.clear();
        count
    }

    async fn submit_transaction_via_rpc(&self, rpc_url: &str, signed_tx: &str) -> Result<String> {
        let client = HttpClient::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .context("Failed to build fallback RPC client")?;

        let payload = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "sendTransaction",
            "params": [
                signed_tx,
                {
                    "encoding": "base64",
                    "skipPreflight": false,
                    "maxRetries": 2
                }
            ]
        });

        let response = client
            .post(rpc_url)
            .json(&payload)
            .send()
            .await
            .context("Fallback RPC submission failed")?;

        let rpc_json: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse fallback RPC response")?;

        if let Some(error) = rpc_json.get("error") {
            return Err(anyhow!("Solana RPC error: {}", error));
        }

        let signature = rpc_json
            .get("result")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("RPC response missing transaction signature"))?;

        Ok(signature.to_string())
    }

    pub async fn get_portfolio_summary(&self) -> HashMap<String, f64> {
        self.active_positions.read().await.clone()
    }
}

pub struct MasterExecutor {
    db: Arc<Database>,
    risk_manager: Arc<RiskManager>,
    trading_executor: TradingExecutor,
    redis_client: RedisClient,
    consumer_group: String,
    consumer_name: String,
    stream_keys: Vec<String>,
}

impl MasterExecutor {
    pub async fn new(db: Arc<Database>, risk_manager: Arc<RiskManager>) -> Result<Self> {
        let paper_trading = std::env::var("PAPER_TRADING_MODE")
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE"))
            .unwrap_or(true);

        let config =
            config::get_config().map_err(|e| anyhow!("Configuration unavailable: {}", e))?;

        let redis_client = RedisClient::open(config.redis_url.clone())
            .map_err(|e| anyhow!("Failed to create Redis client: {}", e))?;

        let live_allowlist = if paper_trading {
            None
        } else {
            let explicit = std::env::var("LIVE_STRATEGIES").ok().and_then(|raw| {
                let parsed: HashSet<String> = raw
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect();
                if parsed.is_empty() {
                    None
                } else {
                    Some(parsed)
                }
            });

            Some(explicit.unwrap_or_else(|| {
                DEFAULT_LIVE_STRATEGIES
                    .iter()
                    .map(|s| s.to_string())
                    .collect()
            }))
        };

        if let Some(ref allowlist) = live_allowlist {
            tracing::info!(
                live_mode = %(!paper_trading),
                strategies = ?allowlist,
                "Live trading allowlist configured"
            );
        }

        let mut trading_executor = TradingExecutor::new(paper_trading, live_allowlist.clone());

        let mut registered_ids: Vec<String> = Vec::new();
        for cfg in live_strategy_configs() {
            let strategy = cfg.instantiate();
            registered_ids.push(cfg.id.to_string());
            trading_executor.add_strategy(cfg.id.to_string(), strategy, cfg.params.clone());
        }

        tracing::info!(registered = ?registered_ids, "Strategies loaded into trading executor");

        if let Some(ref allowlist) = live_allowlist {
            let registered_set: HashSet<String> = registered_ids.iter().cloned().collect();
            let unknown: Vec<String> = allowlist
                .iter()
                .filter(|strategy| !registered_set.contains(*strategy))
                .cloned()
                .collect();
            if !unknown.is_empty() {
                tracing::warn!(unknown = ?unknown, "Allowlist contains unknown strategies");
            }
        }

        Ok(Self {
            db,
            risk_manager,
            trading_executor,
            redis_client,
            consumer_group: "master_executor".to_string(),
            consumer_name: format!("master_executor-{}", Uuid::new_v4()),
            stream_keys: vec![
                "events:price".to_string(),
                "events:volume".to_string(),
                "events:liquidation".to_string(),
                "events:whale".to_string(),
                "events:social".to_string(),
            ],
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        tracing::info!("🚀 MasterExecutor starting main execution loop");

        let mut conn = self
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .context("Failed to obtain Redis connection")?;

        self.ensure_consumer_groups(&mut conn).await?;

        loop {
            match self.read_next_batch(&mut conn).await? {
                Some(reply) => {
                    for stream in reply.keys {
                        let key = stream.key;
                        for entry in stream.ids {
                            let message_id = entry.id;
                            let payload = entry.map;

                            if let Err(err) = self.process_stream_message(&key, &payload).await {
                                tracing::error!(
                                    stream = %key,
                                    id = %message_id,
                                    error = %format!("{err:#}"),
                                    "Failed to process stream message"
                                );
                                continue;
                            }

                            if let Err(e) = redis::cmd("XACK")
                                .arg(&key)
                                .arg(&self.consumer_group)
                                .arg(&message_id)
                                .query_async::<_, i32>(&mut conn)
                                .await
                            {
                                tracing::warn!(
                                    stream = %key,
                                    id = %message_id,
                                    "Failed to acknowledge message: {}",
                                    e
                                );
                            }
                        }
                    }
                }
                None => {
                    sleep(Duration::from_millis(50)).await;
                }
            }
        }
    }

    async fn ensure_consumer_groups(&self, conn: &mut MultiplexedConnection) -> Result<()> {
        for stream in &self.stream_keys {
            let result: redis::RedisResult<String> = redis::cmd("XGROUP")
                .arg("CREATE")
                .arg(stream)
                .arg(&self.consumer_group)
                .arg("$")
                .arg("MKSTREAM")
                .query_async(conn)
                .await;

            if let Err(err) = result {
                let msg = err.to_string();
                if !msg.contains("BUSYGROUP") {
                    return Err(anyhow!(
                        "Failed to ensure consumer group for {}: {}",
                        stream,
                        msg
                    ));
                }
            }
        }

        Ok(())
    }

    async fn read_next_batch(
        &self,
        conn: &mut MultiplexedConnection,
    ) -> Result<Option<StreamReadReply>> {
        let mut cmd = redis::cmd("XREADGROUP");
        cmd.arg("GROUP")
            .arg(&self.consumer_group)
            .arg(&self.consumer_name)
            .arg("COUNT")
            .arg(50)
            .arg("BLOCK")
            .arg(1_000)
            .arg("STREAMS");

        for stream in &self.stream_keys {
            cmd.arg(stream);
        }
        for _ in &self.stream_keys {
            cmd.arg(">");
        }

        match cmd.query_async::<_, StreamReadReply>(conn).await {
            Ok(reply) => Ok(Some(reply)),
            Err(err) => {
                let msg = err.to_string();
                if msg.contains("Timeout") {
                    Ok(None)
                } else {
                    Err(anyhow!("Failed to read from Redis streams: {}", msg))
                }
            }
        }
    }

    async fn process_stream_message(
        &mut self,
        stream_key: &str,
        values: &HashMap<String, Value>,
    ) -> Result<()> {
        let data = Self::extract_string(values, "data")
            .with_context(|| format!("Missing data field in stream {}", stream_key))?;

        let event: Event = serde_json::from_str(&data)
            .with_context(|| format!("Failed to parse event payload on {}", stream_key))?;

        if let Event::Market(market_event) = event {
            // [DEBUG] Log incoming event details
            let event_type = market_event.get_type();
            let token = market_event.token();
            debug!(
                stream = %stream_key,
                event_type = ?event_type,
                token = %token.chars().take(12).collect::<String>(),
                "Processing market event"
            );
            
            let actions = self.trading_executor.process_event(&market_event).await?;
            
            // [DEBUG] Log action count
            if !actions.is_empty() {
                info!(
                    stream = %stream_key,
                    action_count = actions.len(),
                    "Strategies generated {} actions",
                    actions.len()
                );
            }
            
            for (strategy_name, action) in actions {
                self.handle_action(&strategy_name, &market_event, action)
                    .await?;
            }
        }

        Ok(())
    }

    async fn handle_action(
        &mut self,
        strategy_name: &str,
        event: &MarketEvent,
        action: StrategyAction,
    ) -> Result<()> {
        match action {
            StrategyAction::Execute(details) => {
                self.handle_execute_action(strategy_name, event, details)
                    .await
            }
            other => self.trading_executor.execute_action(other).await,
        }
    }

    async fn handle_execute_action(
        &mut self,
        strategy_name: &str,
        event: &MarketEvent,
        details: OrderDetails,
    ) -> Result<()> {
        let price = reference_price(event, &details)
            .ok_or_else(|| anyhow!("No reference price available for {}", strategy_name))?;

        if price <= 0.0 || !price.is_finite() {
            tracing::warn!(
                strategy = %strategy_name,
                symbol = %details.symbol,
                "Skipping trade due to invalid reference price"
            );
            return Ok(());
        }

        if !(0.0..=1.0).contains(&details.confidence) {
            warn!(
                strategy = %strategy_name,
                confidence = details.confidence,
                "Strategy confidence out of bounds"
            );
            return Ok(());
        }

        if details.suggested_size_usd <= 0.0 {
            warn!(
                strategy = %strategy_name,
                size = details.suggested_size_usd,
                "Strategy suggested non-positive trade size"
            );
            return Ok(());
        }

        let quantity = (details.suggested_size_usd / price).max(0.0);
        if quantity <= 0.0 || !quantity.is_finite() {
            tracing::warn!(
                strategy = %strategy_name,
                symbol = %details.symbol,
                "Computed trade quantity is invalid"
            );
            return Ok(());
        }

        let trade = Trade {
            id: Uuid::new_v4().to_string(),
            strategy_id: strategy_name.to_string(),
            symbol: details.symbol.clone(),
            side: details.side,
            quantity,
            price,
            timestamp: event.timestamp(),
            profit_loss: 0.0,
        };

        match self.risk_manager.evaluate_trade(&trade).await? {
            TradeDecision::Allow => {
                self.risk_manager.update_position(&trade).await?;
                self.db.save_trade(&trade).await?;
                self.trading_executor
                    .execute_action(StrategyAction::Execute(details))
                    .await?;
            }
            TradeDecision::Reject {
                event_type,
                severity,
                description,
            } => {
                tracing::warn!(
                    strategy = %strategy_name,
                    symbol = %trade.symbol,
                    reason = %description,
                    "Trade blocked by risk manager"
                );

                let risk_event = RiskEvent {
                    id: Uuid::new_v4().to_string(),
                    event_type,
                    severity,
                    description,
                    timestamp: Utc::now(),
                    strategy_id: Some(strategy_name.to_string()),
                };

                self.db.save_risk_event(&risk_event).await?;
            }
        }

        Ok(())
    }

    fn extract_string(values: &HashMap<String, Value>, key: &str) -> Result<String> {
        let value = values
            .get(key)
            .ok_or_else(|| anyhow!("Missing field {}", key))?;

        match value {
            Value::Data(bytes) => {
                Ok(String::from_utf8(bytes.clone()).context("Invalid UTF-8 in Redis payload")?)
            }
            Value::Status(s) => Ok(s.clone()),
            Value::Okay => Ok("OK".to_string()),
            other => Err(anyhow!("Unsupported Redis value for {}: {:?}", key, other)),
        }
    }
}

fn reference_price(event: &MarketEvent, details: &OrderDetails) -> Option<f64> {
    let candidate = match event {
        MarketEvent::Price(tick) => Some(tick.price_usd),
        MarketEvent::Depth(depth) => Some((depth.bid_price + depth.ask_price) / 2.0),
        MarketEvent::SolPrice(sol) => Some(sol.price_usd),
        _ => details
            .strategy_metadata
            .get("reference_price")
            .and_then(|v| v.as_f64()),
    };

    candidate.filter(|p| p.is_finite() && *p > 0.0)
}
