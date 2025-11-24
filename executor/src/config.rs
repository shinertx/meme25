use serde::Deserialize;
use shared_models::error::{ModelError, Result};
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
    pub birdeye_api_key: Option<String>,
    pub twitter_bearer_token: String,
    pub farcaster_api_key: Option<String>,
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
    pub metrics_port: Option<u16>,
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
            birdeye_api_key: env::var("BIRDEYE_API_KEY").ok(),
            twitter_bearer_token: env::var("TWITTER_BEARER_TOKEN")?,
            farcaster_api_key: env::var("FARCASTER_API_KEY").ok(),
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
            target_annual_return_percent: env::var("TARGET_ANNUAL_RETURN")?
                .parse()
                .unwrap_or(500.0),
            metrics_port: env::var("METRICS_PORT").ok().and_then(|p| p.parse().ok()),
        })
    }

    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn validate(self) -> Result<Self> {
        macro_rules! ensure {
            ($cond:expr, $msg:literal) => {
                if !$cond {
                    return Err(ModelError::Config($msg.into()));
                }
            };
        }

        ensure!(!self.redis_url.is_empty(), "redis_url missing");
        ensure!(!self.database_url.is_empty(), "database_url missing");
        ensure!(
            self.max_position_size_percent > 0.0,
            "max_position_size must be > 0"
        );
        ensure!(
            self.max_daily_drawdown_percent > 0.0 && self.max_daily_drawdown_percent < 100.0,
            "drawdown must be in (0,100) range"
        );
        ensure!(
            self.initial_capital_usd > 0.0,
            "initial_capital must be > 0"
        );
        ensure!(
            self.max_portfolio_size_usd > self.initial_capital_usd,
            "max_portfolio_size must be > initial_capital"
        );

        if let Some(port) = self.metrics_port {
            ensure!(port > 1024, "metrics_port must be > 1024");
        }

        Ok(self)
    }
}

use once_cell::sync::OnceCell;

static CONFIG_CELL: OnceCell<Config> = OnceCell::new();

pub fn get_config() -> Result<&'static Config> {
    CONFIG_CELL.get_or_try_init(|| {
        Config::from_env()
            .map_err(|e| ModelError::Config(format!("Environment variable error: {}", e)))
            .and_then(|config| config.validate())
    })
}

// For backward compatibility with existing code that uses CONFIG
lazy_static::lazy_static! {
    pub static ref CONFIG: Config = {
        get_config().unwrap_or_else(|e| {
            eprintln!("FATAL: Configuration error: {}", e);
            std::process::exit(1);
        }).clone()
    };
}
