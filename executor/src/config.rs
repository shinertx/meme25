use serde::Deserialize;
use std::env;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub solana_rpc_url: String,
    pub solana_ws_url: String,
    pub helius_api_key: String,
    pub jupiter_api_key: String,
    pub pump_fun_api_key: String,
    pub birdeye_api_key: String,
    pub twitter_bearer_token: String,
    pub farcaster_api_key: String,
    pub solana_private_key: String,
    pub initial_capital_usd: f64,
    pub max_portfolio_size_usd: f64,
    pub max_position_size_percent: f64,
    pub max_daily_drawdown_percent: f64,
    pub portfolio_stop_loss_percent: f64,
    pub genetic_population_size: usize,
    pub genetic_mutation_rate: f64,
    pub genetic_crossover_rate: f64,
    pub genetic_elitism_rate: f64,
    pub signer_url: String,
    pub jito_tip_lamports: u64,
    pub max_slippage_percent: f64,
    pub min_liquidity_usd: f64,
    pub target_annual_return_percent: f64,
}

impl Config {
    pub fn from_env() -> Result<Self, env::VarError> {
        Ok(Config {
            database_url: env::var("DATABASE_URL")?,
            redis_url: env::var("REDIS_URL")?,
            solana_rpc_url: env::var("SOLANA_RPC_URL")?,
            solana_ws_url: env::var("SOLANA_WS_URL")?,
            helius_api_key: env::var("HELIUS_API_KEY")?,
            jupiter_api_key: env::var("JUPITER_API_KEY")?,
            pump_fun_api_key: env::var("PUMP_FUN_API_KEY")?,
            birdeye_api_key: env::var("BIRDEYE_API_KEY")?,
            twitter_bearer_token: env::var("TWITTER_BEARER_TOKEN")?,
            farcaster_api_key: env::var("FARCASTER_API_KEY")?,
            solana_private_key: env::var("SOLANA_PRIVATE_KEY")?,
            initial_capital_usd: env::var("INITIAL_CAPITAL")?.parse().unwrap_or(200.0),
            max_portfolio_size_usd: env::var("MAX_PORTFOLIO_SIZE")?.parse().unwrap_or(100000.0),
            max_position_size_percent: env::var("MAX_POSITION_SIZE")?.parse().unwrap_or(20.0),
            max_daily_drawdown_percent: env::var("MAX_DAILY_DRAWDOWN")?.parse().unwrap_or(5.0),
            portfolio_stop_loss_percent: env::var("PORTFOLIO_STOP_LOSS")?.parse().unwrap_or(15.0),
            genetic_population_size: env::var("GENETIC_POPULATION_SIZE")?.parse().unwrap_or(20),
            genetic_mutation_rate: env::var("GENETIC_MUTATION_RATE")?.parse().unwrap_or(0.1),
            genetic_crossover_rate: env::var("GENETIC_CROSSOVER_RATE")?.parse().unwrap_or(0.7),
            genetic_elitism_rate: env::var("GENETIC_ELITISM_RATE")?.parse().unwrap_or(0.2),
            signer_url: env::var("SIGNER_URL")?,
            jito_tip_lamports: env::var("JITO_TIP_LAMPORTS")?.parse().unwrap_or(100000),
            max_slippage_percent: env::var("MAX_SLIPPAGE")?.parse().unwrap_or(2.0),
            min_liquidity_usd: env::var("MIN_LIQUIDITY")?.parse().unwrap_or(10000.0),
            target_annual_return_percent: env::var("TARGET_ANNUAL_RETURN")?.parse().unwrap_or(500.0),
        })
    }
}

lazy_static::lazy_static! {
    pub static ref CONFIG: Config = Config::from_env().expect("Failed to load configuration");
}
