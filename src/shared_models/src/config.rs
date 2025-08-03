use crate::error::ModelError;

impl Config {
    pub fn from_env() -> Result<Self, ModelError> {
        let config = Config {
            // ...existing code...
        };
        
        config.validate()
    }
    
    pub fn validate(self) -> Result<Self, ModelError> {
        macro_rules! ensure { 
            ($cond:expr, $msg:literal) => { 
                if !$cond { 
                    return Err(ModelError::Config($msg.into())); 
                } 
            }; 
        }
        
        // Essential configuration validation
        ensure!(!self.redis_url.is_empty(), "redis_url missing");
        ensure!(!self.database_url.is_empty(), "database_url missing");
        ensure!(self.max_position_size > 0.0, "max_position_size must be > 0");
        ensure!(self.max_daily_drawdown > 0.0 && self.max_daily_drawdown < 1.0, 
            "drawdown must be between 0 and 1");
        ensure!(self.metrics_port > 0 && self.metrics_port < 65536, 
            "metrics_port invalid");
        
        // Trading configuration validation
        ensure!(self.min_order_size >= 1.0, "min_order_size must be >= $1");
        ensure!(self.max_order_size <= 50.0, "max_order_size must be <= $50 for $200 capital");
        ensure!(self.max_open_positions > 0 && self.max_open_positions <= 10,
            "max_open_positions must be 1-10");
        
        // API configuration validation
        ensure!(!self.helius_api_key.is_empty(), "helius_api_key required");
        ensure!(!self.jupiter_api_url.is_empty(), "jupiter_api_url required");
        
        Ok(self)
    }
}