use shared_models::error::{Result, ModelError};
use tracing::{info, warn, debug, error};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};
use tokio::sync::{broadcast, RwLock};
use std::collections::HashMap;
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebSocketMessageType {
    // Client -> Server
    Subscribe,
    Unsubscribe,
    GetStatus,
    GetHistory,
    
    // Server -> Client
    Portfolio,
    Strategy,
    Risk,
    Market,
    Trade,
    Alert,
    Opportunity,
    CircuitBreaker,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMessage {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub message_type: WebSocketMessageType,
    pub data: serde_json::Value,
    pub subscription_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionRequest {
    pub subscription_id: String,
    pub channels: Vec<String>, // "portfolio", "strategies", "risk", "market", "trades", "alerts"
    pub filters: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioUpdate {
    pub timestamp: DateTime<Utc>,
    pub total_pnl_usd: f64,
    pub daily_pnl_usd: f64,
    pub unrealized_pnl_usd: f64,
    pub portfolio_value: f64,
    pub drawdown_pct: f64,
    pub active_positions: u32,
    pub daily_volume: f64,
    pub win_rate: f64,
    pub sharpe_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyUpdate {
    pub timestamp: DateTime<Utc>,
    pub strategy_id: String,
    pub strategy_type: String,
    pub pnl_usd: f64,
    pub daily_pnl_usd: f64,
    pub volume_usd: f64,
    pub win_rate: f64,
    pub sharpe_ratio: f64,
    pub active_positions: u32,
    pub status: String, // "Active", "Paused", "Stopped"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskUpdate {
    pub timestamp: DateTime<Utc>,
    pub portfolio_var_95: f64,
    pub portfolio_volatility: f64,
    pub correlation_risk_score: f64,
    pub concentration_risk_score: f64,
    pub margin_utilization_pct: f64,
    pub circuit_breaker_triggered: bool,
    pub risk_level: String, // "Low", "Medium", "High", "Critical"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketUpdate {
    pub timestamp: DateTime<Utc>,
    pub active_tokens: u32,
    pub avg_volatility_pct: f64,
    pub market_momentum_score: f64,
    pub social_sentiment_score: f64,
    pub opportunity_count: u32,
    pub market_regime: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeUpdate {
    pub timestamp: DateTime<Utc>,
    pub trade_id: String,
    pub strategy_id: String,
    pub symbol: String,
    pub side: String, // "Buy", "Sell"
    pub size_usd: f64,
    pub price: f64,
    pub pnl_usd: Option<f64>,
    pub status: String, // "Executed", "Partial", "Failed"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertUpdate {
    pub timestamp: DateTime<Utc>,
    pub alert_id: String,
    pub level: String, // "Info", "Warning", "Critical"
    pub category: String, // "Risk", "Performance", "System", "Market"
    pub message: String,
    pub strategy_id: Option<String>,
    pub symbol: Option<String>,
    pub auto_dismiss_after_seconds: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpportunityUpdate {
    pub timestamp: DateTime<Utc>,
    pub opportunity_id: String,
    pub symbol: String,
    pub opportunity_type: String,
    pub confidence_score: f64,
    pub potential_return_pct: f64,
    pub risk_score: f64,
    pub time_horizon_minutes: u32,
    pub strategies_interested: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerUpdate {
    pub timestamp: DateTime<Utc>,
    pub breaker_name: String,
    pub trigger_value: f64,
    pub threshold: f64,
    pub severity: String, // "Warning", "Throttle", "Pause", "Stop", "Emergency"
    pub message: String,
    pub affected_strategies: Vec<String>,
    pub recovery_eta: Option<DateTime<Utc>>,
}

struct ClientConnection {
    subscriptions: HashMap<String, Vec<String>>, // subscription_id -> channels
    last_activity: DateTime<Utc>,
}

pub struct WebSocketServer {
    listener: Option<TcpListener>,
    clients: Arc<RwLock<HashMap<String, ClientConnection>>>,
    
    // Broadcast channels for real-time updates
    portfolio_tx: broadcast::Sender<PortfolioUpdate>,
    strategy_tx: broadcast::Sender<StrategyUpdate>,
    risk_tx: broadcast::Sender<RiskUpdate>,
    market_tx: broadcast::Sender<MarketUpdate>,
    trade_tx: broadcast::Sender<TradeUpdate>,
    alert_tx: broadcast::Sender<AlertUpdate>,
    opportunity_tx: broadcast::Sender<OpportunityUpdate>,
    circuit_breaker_tx: broadcast::Sender<CircuitBreakerUpdate>,
    
    // Configuration
    port: u16,
    max_clients: usize,
    heartbeat_interval_seconds: u64,
    client_timeout_seconds: u64,
    
    // Metrics
    total_connections: u64,
    messages_sent: u64,
    messages_received: u64,
}

impl WebSocketServer {
    pub fn new(port: u16) -> Self {
        let (portfolio_tx, _) = broadcast::channel(1000);
        let (strategy_tx, _) = broadcast::channel(1000);
        let (risk_tx, _) = broadcast::channel(1000);
        let (market_tx, _) = broadcast::channel(1000);
        let (trade_tx, _) = broadcast::channel(1000);
        let (alert_tx, _) = broadcast::channel(1000);
        let (opportunity_tx, _) = broadcast::channel(1000);
        let (circuit_breaker_tx, _) = broadcast::channel(1000);

        Self {
            listener: None,
            clients: Arc::new(RwLock::new(HashMap::new())),
            portfolio_tx,
            strategy_tx,
            risk_tx,
            market_tx,
            trade_tx,
            alert_tx,
            opportunity_tx,
            circuit_breaker_tx,
            port,
            max_clients: 100,
            heartbeat_interval_seconds: 30,
            client_timeout_seconds: 300, // 5 minutes
            total_connections: 0,
            messages_sent: 0,
            messages_received: 0,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        let addr = format!("127.0.0.1:{}", self.port);
        let listener = TcpListener::bind(&addr).await
            .map_err(|e| ModelError::Network(format!("Failed to bind WebSocket server: {}", e)))?;
        info!("WebSocket server listening on {}", addr);
        
        self.listener = Some(listener);
        
        // Start heartbeat task
        self.start_heartbeat_task().await;
        
        // Start connection handler
        self.handle_connections().await?;
        
        Ok(())
    }

    async fn handle_connections(&mut self) -> Result<()> {
        if let Some(listener) = &self.listener {
            while let Ok((stream, addr)) = listener.accept().await {
                // Check client limit
                if self.clients.read().await.len() >= self.max_clients {
                    warn!("Rejecting connection from {} - max clients reached", addr);
                    continue;
                }

                info!("New WebSocket connection from {}", addr);
                self.total_connections += 1;

                let clients = self.clients.clone();
                let portfolio_rx = self.portfolio_tx.subscribe();
                let strategy_rx = self.strategy_tx.subscribe();
                let risk_rx = self.risk_tx.subscribe();
                let market_rx = self.market_tx.subscribe();
                let trade_rx = self.trade_tx.subscribe();
                let alert_rx = self.alert_tx.subscribe();
                let opportunity_rx = self.opportunity_tx.subscribe();
                let circuit_breaker_rx = self.circuit_breaker_tx.subscribe();

                tokio::spawn(async move {
                    if let Err(e) = Self::handle_client(
                        stream,
                        clients,
                        portfolio_rx,
                        strategy_rx,
                        risk_rx,
                        market_rx,
                        trade_rx,
                        alert_rx,
                        opportunity_rx,
                        circuit_breaker_rx,
                    ).await {
                        error!("Error handling client {}: {}", addr, e);
                    }
                });
            }
        }
        Ok(())
    }

    async fn handle_client(
        stream: TcpStream,
        clients: Arc<RwLock<HashMap<String, ClientConnection>>>,
        mut portfolio_rx: broadcast::Receiver<PortfolioUpdate>,
        mut strategy_rx: broadcast::Receiver<StrategyUpdate>,
        mut risk_rx: broadcast::Receiver<RiskUpdate>,
        mut market_rx: broadcast::Receiver<MarketUpdate>,
        mut trade_rx: broadcast::Receiver<TradeUpdate>,
        mut alert_rx: broadcast::Receiver<AlertUpdate>,
        mut opportunity_rx: broadcast::Receiver<OpportunityUpdate>,
        mut circuit_breaker_rx: broadcast::Receiver<CircuitBreakerUpdate>,
    ) -> Result<()> {
        let ws_stream = accept_async(stream).await
            .map_err(|e| ModelError::Network(format!("Failed to accept WebSocket connection: {}", e)))?;
        let client_id = Uuid::new_v4().to_string();
        
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        
        // Create client connection
        let client_connection = ClientConnection {
            subscriptions: HashMap::new(),
            last_activity: Utc::now(),
        };
        
        // Store client
        clients.write().await.insert(client_id.clone(), client_connection);
        
        // Send welcome message
        let welcome_msg = WebSocketMessage {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            message_type: WebSocketMessageType::Portfolio,
            data: serde_json::json!({
                "status": "connected",
                "client_id": client_id,
                "server_version": "1.0.0",
                "available_channels": ["portfolio", "strategies", "risk", "market", "trades", "alerts", "opportunities", "circuit_breakers"]
            }),
            subscription_id: None,
        };
        
        if let Ok(msg_text) = serde_json::to_string(&welcome_msg) {
            let _ = ws_sender.send(Message::Text(msg_text)).await;
        }
        
        // Handle client messages and broadcasts
        loop {
            tokio::select! {
                // Handle incoming client messages
                msg = ws_receiver.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            if let Err(e) = Self::handle_client_message(&client_id, &text, &clients).await {
                                error!("Error handling client message: {}", e);
                            }
                        },
                        Some(Ok(Message::Binary(_))) => {
                            // Binary messages not supported in this implementation
                            debug!("Received binary message from client {}, ignoring", client_id);
                        },
                        Some(Ok(Message::Ping(payload))) => {
                            // Respond to ping with pong
                            if let Err(e) = ws_sender.send(Message::Pong(payload)).await {
                                error!("Failed to send pong to client {}: {}", client_id, e);
                                break;
                            }
                        },
                        Some(Ok(Message::Pong(_))) => {
                            // Pong received, connection is alive
                            debug!("Received pong from client {}", client_id);
                        },
                        Some(Ok(Message::Close(_))) => {
                            info!("Client {} disconnected", client_id);
                            break;
                        },
                        Some(Ok(Message::Frame(_))) => {
                            // Raw frames not handled in this implementation
                            debug!("Received raw frame from client {}, ignoring", client_id);
                        },
                        Some(Err(e)) => {
                            error!("WebSocket error for client {}: {}", client_id, e);
                            break;
                        },
                        None => break,
                    }
                },
                
                // Handle portfolio updates
                portfolio_update = portfolio_rx.recv() => {
                    if let Ok(update) = portfolio_update {
                        Self::broadcast_to_subscribed_clients(
                            &clients,
                            &client_id,
                            "portfolio",
                            WebSocketMessageType::Portfolio,
                            &update,
                            &mut ws_sender,
                        ).await;
                    }
                },
                
                // Handle strategy updates
                strategy_update = strategy_rx.recv() => {
                    if let Ok(update) = strategy_update {
                        Self::broadcast_to_subscribed_clients(
                            &clients,
                            &client_id,
                            "strategies",
                            WebSocketMessageType::Strategy,
                            &update,
                            &mut ws_sender,
                        ).await;
                    }
                },
                
                // Handle risk updates
                risk_update = risk_rx.recv() => {
                    if let Ok(update) = risk_update {
                        Self::broadcast_to_subscribed_clients(
                            &clients,
                            &client_id,
                            "risk",
                            WebSocketMessageType::Risk,
                            &update,
                            &mut ws_sender,
                        ).await;
                    }
                },
                
                // Handle market updates
                market_update = market_rx.recv() => {
                    if let Ok(update) = market_update {
                        Self::broadcast_to_subscribed_clients(
                            &clients,
                            &client_id,
                            "market",
                            WebSocketMessageType::Market,
                            &update,
                            &mut ws_sender,
                        ).await;
                    }
                },
                
                // Handle trade updates
                trade_update = trade_rx.recv() => {
                    if let Ok(update) = trade_update {
                        Self::broadcast_to_subscribed_clients(
                            &clients,
                            &client_id,
                            "trades",
                            WebSocketMessageType::Trade,
                            &update,
                            &mut ws_sender,
                        ).await;
                    }
                },
                
                // Handle alert updates
                alert_update = alert_rx.recv() => {
                    if let Ok(update) = alert_update {
                        Self::broadcast_to_subscribed_clients(
                            &clients,
                            &client_id,
                            "alerts",
                            WebSocketMessageType::Alert,
                            &update,
                            &mut ws_sender,
                        ).await;
                    }
                },
                
                // Handle opportunity updates
                opportunity_update = opportunity_rx.recv() => {
                    if let Ok(update) = opportunity_update {
                        Self::broadcast_to_subscribed_clients(
                            &clients,
                            &client_id,
                            "opportunities",
                            WebSocketMessageType::Opportunity,
                            &update,
                            &mut ws_sender,
                        ).await;
                    }
                },
                
                // Handle circuit breaker updates
                circuit_breaker_update = circuit_breaker_rx.recv() => {
                    if let Ok(update) = circuit_breaker_update {
                        Self::broadcast_to_subscribed_clients(
                            &clients,
                            &client_id,
                            "circuit_breakers",
                            WebSocketMessageType::CircuitBreaker,
                            &update,
                            &mut ws_sender,
                        ).await;
                    }
                },
            }
        }
        
        // Remove client on disconnect
        clients.write().await.remove(&client_id);
        
        Ok(())
    }

    async fn handle_client_message(
        client_id: &str,
        message: &str,
        clients: &Arc<RwLock<HashMap<String, ClientConnection>>>,
    ) -> Result<()> {
        debug!("Received message from client {}: {}", client_id, message);
        
        match serde_json::from_str::<WebSocketMessage>(message) {
            Ok(ws_msg) => {
                match ws_msg.message_type {
                    WebSocketMessageType::Subscribe => {
                        if let Ok(sub_req) = serde_json::from_value::<SubscriptionRequest>(ws_msg.data) {
                            // Update client subscriptions
                            if let Some(client) = clients.write().await.get_mut(client_id) {
                                client.subscriptions.insert(sub_req.subscription_id.clone(), sub_req.channels.clone());
                                client.last_activity = Utc::now();
                                info!("Client {} subscribed to channels: {:?}", client_id, sub_req.channels);
                            }
                        }
                    },
                    WebSocketMessageType::Unsubscribe => {
                        if let Ok(sub_id) = serde_json::from_value::<String>(ws_msg.data) {
                            if let Some(client) = clients.write().await.get_mut(client_id) {
                                client.subscriptions.remove(&sub_id);
                                client.last_activity = Utc::now();
                                info!("Client {} unsubscribed from {}", client_id, sub_id);
                            }
                        }
                    },
                    _ => {
                        // Update activity timestamp for any message
                        if let Some(client) = clients.write().await.get_mut(client_id) {
                            client.last_activity = Utc::now();
                        }
                    }
                }
            },
            Err(e) => {
                warn!("Invalid message from client {}: {}", client_id, e);
            }
        }
        
        Ok(())
    }

    async fn broadcast_to_subscribed_clients<T: Serialize>(
        clients: &Arc<RwLock<HashMap<String, ClientConnection>>>,
        current_client_id: &str,
        channel: &str,
        msg_type: WebSocketMessageType,
        data: &T,
        ws_sender: &mut futures_util::stream::SplitSink<WebSocketStream<TcpStream>, Message>,
    ) {
        // Check if current client is subscribed to this channel
        let is_subscribed = {
            if let Some(client) = clients.read().await.get(current_client_id) {
                client.subscriptions.values().any(|channels| channels.contains(&channel.to_string()))
            } else {
                false
            }
        };

        if is_subscribed {
            let ws_msg = WebSocketMessage {
                id: Uuid::new_v4().to_string(),
                timestamp: Utc::now(),
                message_type: msg_type,
                data: serde_json::to_value(data).unwrap_or_default(),
                subscription_id: None,
            };

            if let Ok(msg_text) = serde_json::to_string(&ws_msg) {
                if let Err(e) = ws_sender.send(Message::Text(msg_text)).await {
                    error!("Failed to send message to client {}: {}", current_client_id, e);
                }
            }
        }
    }

    async fn start_heartbeat_task(&self) {
        let clients = self.clients.clone();
        let interval = self.heartbeat_interval_seconds;
        let timeout = self.client_timeout_seconds;

        tokio::spawn(async move {
            let mut heartbeat_interval = tokio::time::interval(
                tokio::time::Duration::from_secs(interval)
            );

            loop {
                heartbeat_interval.tick().await;
                
                let now = Utc::now();
                let mut clients_to_remove = Vec::new();
                
                // Check for inactive clients
                {
                    let clients_read = clients.read().await;
                    for (client_id, client) in clients_read.iter() {
                        let inactive_duration = now.signed_duration_since(client.last_activity);
                        if inactive_duration.num_seconds() > timeout as i64 {
                            clients_to_remove.push(client_id.clone());
                        }
                    }
                }
                
                // Remove inactive clients
                if !clients_to_remove.is_empty() {
                    let mut clients_write = clients.write().await;
                    for client_id in clients_to_remove {
                        clients_write.remove(&client_id);
                        info!("Removed inactive client: {}", client_id);
                    }
                }
                
                debug!("Active WebSocket clients: {}", clients.read().await.len());
            }
        });
    }

    // Public methods to broadcast updates
    pub async fn broadcast_portfolio_update(&self, update: PortfolioUpdate) -> Result<()> {
        match self.portfolio_tx.send(update) {
            Ok(_) => {
                debug!("Broadcasted portfolio update to {} subscribers", self.portfolio_tx.receiver_count());
                Ok(())
            },
            Err(e) => {
                warn!("Failed to broadcast portfolio update: {}", e);
                Ok(()) // Don't fail the system if no subscribers
            }
        }
    }

    pub async fn broadcast_strategy_update(&self, update: StrategyUpdate) -> Result<()> {
        match self.strategy_tx.send(update) {
            Ok(_) => {
                debug!("Broadcasted strategy update to {} subscribers", self.strategy_tx.receiver_count());
                Ok(())
            },
            Err(e) => {
                warn!("Failed to broadcast strategy update: {}", e);
                Ok(())
            }
        }
    }

    pub async fn broadcast_risk_update(&self, update: RiskUpdate) -> Result<()> {
        match self.risk_tx.send(update) {
            Ok(_) => {
                debug!("Broadcasted risk update to {} subscribers", self.risk_tx.receiver_count());
                Ok(())
            },
            Err(e) => {
                warn!("Failed to broadcast risk update: {}", e);
                Ok(())
            }
        }
    }

    pub async fn broadcast_market_update(&self, update: MarketUpdate) -> Result<()> {
        match self.market_tx.send(update) {
            Ok(_) => {
                debug!("Broadcasted market update to {} subscribers", self.market_tx.receiver_count());
                Ok(())
            },
            Err(e) => {
                warn!("Failed to broadcast market update: {}", e);
                Ok(())
            }
        }
    }

    pub async fn broadcast_trade_update(&self, update: TradeUpdate) -> Result<()> {
        match self.trade_tx.send(update) {
            Ok(_) => {
                debug!("Broadcasted trade update to {} subscribers", self.trade_tx.receiver_count());
                Ok(())
            },
            Err(e) => {
                warn!("Failed to broadcast trade update: {}", e);
                Ok(())
            }
        }
    }

    pub async fn broadcast_alert_update(&self, update: AlertUpdate) -> Result<()> {
        match self.alert_tx.send(update) {
            Ok(_) => {
                debug!("Broadcasted alert update to {} subscribers", self.alert_tx.receiver_count());
                Ok(())
            },
            Err(e) => {
                warn!("Failed to broadcast alert update: {}", e);
                Ok(())
            }
        }
    }

    pub async fn broadcast_opportunity_update(&self, update: OpportunityUpdate) -> Result<()> {
        match self.opportunity_tx.send(update) {
            Ok(_) => {
                debug!("Broadcasted opportunity update to {} subscribers", self.opportunity_tx.receiver_count());
                Ok(())
            },
            Err(e) => {
                warn!("Failed to broadcast opportunity update: {}", e);
                Ok(())
            }
        }
    }

    pub async fn broadcast_circuit_breaker_update(&self, update: CircuitBreakerUpdate) -> Result<()> {
        match self.circuit_breaker_tx.send(update) {
            Ok(_) => {
                debug!("Broadcasted circuit breaker update to {} subscribers", self.circuit_breaker_tx.receiver_count());
                Ok(())
            },
            Err(e) => {
                warn!("Failed to broadcast circuit breaker update: {}", e);
                Ok(())
            }
        }
    }

    pub async fn get_connection_stats(&self) -> serde_json::Value {
        let clients_read = self.clients.read().await;
        serde_json::json!({
            "active_connections": clients_read.len(),
            "total_connections": self.total_connections,
            "messages_sent": self.messages_sent,
            "messages_received": self.messages_received,
            "max_clients": self.max_clients,
            "port": self.port,
            "subscribers": {
                "portfolio": self.portfolio_tx.receiver_count(),
                "strategy": self.strategy_tx.receiver_count(),
                "risk": self.risk_tx.receiver_count(),
                "market": self.market_tx.receiver_count(),
                "trade": self.trade_tx.receiver_count(),
                "alert": self.alert_tx.receiver_count(),
                "opportunity": self.opportunity_tx.receiver_count(),
                "circuit_breaker": self.circuit_breaker_tx.receiver_count(),
            }
        })
    }
}
