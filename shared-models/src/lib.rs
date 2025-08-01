use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

// Event Types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventType {
    Price,
    Social,
    Depth,
    Bridge,
    Funding,
    OnChain,
    SolPrice,
    TwitterRaw,
    FarcasterRaw,
    Whale,
    Liquidation,
    Airdrop,
    Volume,
}

impl EventType {
    pub fn to_string(&self) -> &'static str {
        match self {
            EventType::Price => "price",
            EventType::Social => "social",
            EventType::Depth => "depth",
            EventType::Bridge => "bridge",
            EventType::Funding => "funding",
            EventType::OnChain => "onchain",
            EventType::SolPrice => "sol_price",
            EventType::TwitterRaw => "twitter_raw",
            EventType::FarcasterRaw => "farcaster_raw",
            EventType::Whale => "whale",
            EventType::Liquidation => "liquidation",
            EventType::Airdrop => "airdrop",
            EventType::Volume => "volume",
        }
    }
}

// Market Events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketEvent {
    Price(PriceTick),
    Social(SocialMention),
    Depth(DepthEvent),
    Bridge(BridgeEvent),
    Funding(FundingEvent),
    OnChain(OnChainEvent),
    SolPrice(SolPriceEvent),
    TwitterRaw(TwitterRawEvent),
    FarcasterRaw(FarcasterRawEvent),
    Whale(WhaleEvent),
    Liquidation(LiquidationEvent),
    Airdrop(AirdropEvent),
    Volume(VolumeEvent),
}

impl MarketEvent {
    pub fn get_type(&self) -> EventType {
        match self {
            MarketEvent::Price(_) => EventType::Price,
            MarketEvent::Social(_) => EventType::Social,
            MarketEvent::Depth(_) => EventType::Depth,
            MarketEvent::Bridge(_) => EventType::Bridge,
            MarketEvent::Funding(_) => EventType::Funding,
            MarketEvent::OnChain(_) => EventType::OnChain,
            MarketEvent::SolPrice(_) => EventType::SolPrice,
            MarketEvent::TwitterRaw(_) => EventType::TwitterRaw,
            MarketEvent::FarcasterRaw(_) => EventType::FarcasterRaw,
            MarketEvent::Whale(_) => EventType::Whale,
            MarketEvent::Liquidation(_) => EventType::Liquidation,
            MarketEvent::Airdrop(_) => EventType::Airdrop,
            MarketEvent::Volume(_) => EventType::Volume,
        }
    }
    
    pub fn token(&self) -> &str {
        match self {
            MarketEvent::Price(e) => &e.token_address,
            MarketEvent::Social(e) => &e.token_address,
            MarketEvent::Depth(e) => &e.token_address,
            MarketEvent::Bridge(e) => &e.token_address,
            MarketEvent::Funding(e) => &e.token_address,
            MarketEvent::OnChain(e) => &e.token_address,
            MarketEvent::SolPrice(_) => "SOL",
            MarketEvent::TwitterRaw(_) => "",
            MarketEvent::FarcasterRaw(_) => "",
            MarketEvent::Whale(e) => &e.token_address,
            MarketEvent::Liquidation(e) => &e.token_address,
            MarketEvent::Airdrop(e) => &e.token_address,
            MarketEvent::Volume(e) => &e.token_address,
        }
    }
    
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            MarketEvent::Price(e) => e.timestamp,
            MarketEvent::Social(e) => e.timestamp,
            MarketEvent::Depth(e) => e.timestamp,
            MarketEvent::Bridge(e) => e.timestamp,
            MarketEvent::Funding(e) => e.timestamp,
            MarketEvent::OnChain(e) => e.timestamp,
            MarketEvent::SolPrice(e) => e.timestamp,
            MarketEvent::TwitterRaw(e) => DateTime::from_timestamp(e.timestamp, 0).unwrap_or_else(Utc::now),
            MarketEvent::FarcasterRaw(e) => DateTime::from_timestamp(e.timestamp, 0).unwrap_or_else(Utc::now),
            MarketEvent::Whale(e) => e.timestamp,
            MarketEvent::Liquidation(e) => e.timestamp,
            MarketEvent::Airdrop(e) => e.timestamp,
            MarketEvent::Volume(e) => e.timestamp,
        }
    }
}

// Event Structs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceTick {
    pub token_address: String,
    pub price_usd: f64,
    pub volume_usd_1m: f64,
    pub volume_usd_5m: f64,
    pub volume_usd_15m: f64,
    pub price_change_1m: f64,
    pub price_change_5m: f64,
    pub liquidity_usd: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialMention {
    pub token_address: String,
    pub source: String,
    pub sentiment: f64,
    pub engagement_score: f64,
    pub influencer_score: f64,
    pub mentions_1h: u32,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthEvent {
    pub token_address: String,
    pub bid_price: f64,
    pub ask_price: f64,
    pub bid_size_usd: f64,
    pub ask_size_usd: f64,
    pub spread_bps: f64,
    pub imbalance_ratio: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeEvent {
    pub token_address: String,
    pub source_chain: String,
    pub destination_chain: String,
    pub volume_usd: f64,
    pub unique_users: u32,
    pub avg_transfer_size: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingEvent {
    pub token_address: String,
    pub funding_rate_pct: f64,
    pub open_interest_usd: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnChainEvent {
    pub token_address: String,
    pub event_type: String,
    pub details: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolPriceEvent {
    pub price_usd: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwitterRawEvent {
    pub tweet_id: String,
    pub text: String,
    pub author_id: String,
    pub author_followers: u32,
    pub engagement_rate: f64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FarcasterRawEvent {
    pub cast_hash: String,
    pub text: String,
    pub author_fid: String,
    pub author_followers: u32,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhaleEvent {
    pub token_address: String,
    pub wallet_address: String,
    pub action: String, // "buy", "sell", "transfer"
    pub amount_usd: f64,
    pub amount_tokens: f64,
    pub wallet_balance_usd: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationEvent {
    pub token_address: String,
    pub liquidated_amount_usd: f64,
    pub liquidation_price: f64,
    pub platform: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirdropEvent {
    pub token_address: String,
    pub recipients_count: u32,
    pub total_amount_usd: f64,
    pub avg_per_wallet: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeEvent {
    pub token_address: String,
    pub volume_spike_ratio: f64, // Current vs average
    pub buy_volume_usd: f64,
    pub sell_volume_usd: f64,
    pub unique_traders: u32,
    pub large_trades_count: u32,
    pub timestamp: DateTime<Utc>,
}

// Trading Types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Long,
    Short,
}

impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Side::Long => write!(f, "Long"),
            Side::Short => write!(f, "Short"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeMode {
    Simulating,
    Paper,
    Live,
}

// Strategy Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderDetails {
    pub token_address: String,
    pub symbol: String,
    pub suggested_size_usd: f64,
    pub confidence: f64,
    pub side: Side,
    pub strategy_metadata: HashMap<String, serde_json::Value>,
    pub risk_metrics: RiskMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetrics {
    pub position_size_pct: f64,
    pub stop_loss_price: Option<f64>,
    pub take_profit_price: Option<f64>,
    pub max_slippage_bps: u16,
    pub time_limit_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StrategyAction {
    Execute(OrderDetails),
    Hold,
    ReducePosition(f64), // Percentage to reduce
    ClosePosition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyAllocation {
    pub id: String,
    pub weight: f64,
    pub sharpe_ratio: f64,
    pub mode: TradeMode,
    pub params: serde_json::Value,
    pub capital_allocated: f64,
    pub max_position_usd: f64,
    pub current_positions: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategySpec {
    pub id: String,
    pub family: String,
    pub params: serde_json::Value,
    pub fitness: f64,
    pub created_at: DateTime<Utc>,
}

// Performance Tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPerformance {
    pub strategy_id: String,
    pub total_trades: u32,
    pub winning_trades: u32,
    pub total_pnl_usd: f64,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub max_drawdown_pct: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub avg_win_usd: f64,
    pub avg_loss_usd: f64,
    pub last_updated: DateTime<Utc>,
}

// Signer Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRequest {
    pub transaction_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignResponse {
    pub signed_transaction_b64: String,
}

// Risk Management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioState {
    pub total_capital: f64,
    pub available_capital: f64,
    pub positions_value: f64,
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,
    pub daily_pnl: f64,
    pub max_drawdown_today: f64,
    pub position_count: u32,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLimits {
    pub max_position_size_usd: f64,
    pub max_daily_drawdown_pct: f64,
    pub max_position_count: u32,
    pub max_correlation: f64,
    pub min_liquidity_usd: f64,
    pub max_slippage_bps: u16,
}

// Backtest Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestRequest {
    pub strategy_spec: StrategySpec,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub initial_capital: f64,
    pub max_position_size: f64,
    pub data_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    pub backtest_id: String,
    pub strategy_id: String,
    pub status: String,
    pub total_return_pct: f64,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub max_drawdown_pct: f64,
    pub total_trades: u32,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub avg_trade_duration_seconds: u64,
    pub metadata: serde_json::Value,
}

// Strategy trait for the execution engine
use async_trait::async_trait;
use std::collections::HashSet;

#[async_trait]
pub trait Strategy: Send + Sync {
    fn id(&self) -> &'static str;
    fn subscriptions(&self) -> HashSet<EventType>;
    async fn init(&mut self, params: &serde_json::Value) -> anyhow::Result<()>;
    async fn on_event(&mut self, event: &MarketEvent) -> anyhow::Result<StrategyAction>;
    fn get_state(&self) -> serde_json::Value;
}
