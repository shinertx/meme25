use chrono::{DateTime, Utc, Duration};
use shared_models::error::{Result, ModelError};
use tracing::{warn, debug};

pub struct DataFreshnessValidator {
    max_age_ms: u64,
    max_price_deviation: f64,
}

impl DataFreshnessValidator {
    pub fn new(max_age_ms: u64, max_price_deviation: f64) -> Self {
        Self {
            max_age_ms,
            max_price_deviation,
        }
    }

    /// Validates if market data is fresh enough for trading decisions
    pub fn validate_market_data_age(&self, timestamp: DateTime<Utc>) -> Result<()> {
        let now = Utc::now();
        let age = now.signed_duration_since(timestamp);
        let max_age = Duration::milliseconds(self.max_age_ms as i64);

        if age > max_age {
            warn!(
                data_age_ms = age.num_milliseconds(),
                max_age_ms = self.max_age_ms,
                "Market data is stale, rejecting"
            );
            return Err(ModelError::Strategy(format!(
                "Market data too old: {}ms (max: {}ms)", 
                age.num_milliseconds(), 
                self.max_age_ms
            )));
        }

        debug!(
            data_age_ms = age.num_milliseconds(),
            "Market data freshness validated"
        );
        
        Ok(())
    }

    /// Validates price data for sudden spikes that might indicate bad data
    pub fn validate_price_deviation(&self, current_price: f64, previous_price: f64) -> Result<()> {
        if previous_price <= 0.0 || current_price <= 0.0 {
            return Err(ModelError::Strategy("Invalid price data (non-positive)".into()));
        }

        let deviation = (current_price - previous_price).abs() / previous_price;
        
        if deviation > self.max_price_deviation {
            warn!(
                current_price = current_price,
                previous_price = previous_price,
                deviation_pct = deviation * 100.0,
                max_deviation_pct = self.max_price_deviation * 100.0,
                "Price deviation too large, possible bad data"
            );
            return Err(ModelError::Strategy(format!(
                "Price deviation too large: {:.2}% (max: {:.2}%)",
                deviation * 100.0,
                self.max_price_deviation * 100.0
            )));
        }

        Ok(())
    }

    /// Comprehensive validation for price data
    pub fn validate_price_data(
        &self, 
        current_price: f64, 
        timestamp: DateTime<Utc>, 
        previous_price: Option<f64>
    ) -> Result<()> {
        // Check data freshness
        self.validate_market_data_age(timestamp)?;

        // Check for valid price
        if current_price <= 0.0 || !current_price.is_finite() {
            return Err(ModelError::Strategy(format!(
                "Invalid price value: {}", current_price
            )));
        }

        // Check price deviation if we have previous data
        if let Some(prev_price) = previous_price {
            self.validate_price_deviation(current_price, prev_price)?;
        }

        Ok(())
    }

    /// Validates volume data
    pub fn validate_volume_data(&self, volume: f64, timestamp: DateTime<Utc>) -> Result<()> {
        // Check data freshness
        self.validate_market_data_age(timestamp)?;

        // Check for valid volume
        if volume < 0.0 || !volume.is_finite() {
            return Err(ModelError::Strategy(format!(
                "Invalid volume value: {}", volume
            )));
        }

        Ok(())
    }

    /// Validates liquidity data
    pub fn validate_liquidity_data(&self, liquidity_usd: f64, timestamp: DateTime<Utc>) -> Result<()> {
        // Check data freshness
        self.validate_market_data_age(timestamp)?;

        // Check for valid liquidity
        if liquidity_usd < 0.0 || !liquidity_usd.is_finite() {
            return Err(ModelError::Strategy(format!(
                "Invalid liquidity value: {}", liquidity_usd
            )));
        }

        Ok(())
    }
}

impl Default for DataFreshnessValidator {
    fn default() -> Self {
        Self::new(
            500, // 500ms max age (as per .env)
            0.05 // 5% max price deviation (as per .env)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_fresh_data_passes() {
        let validator = DataFreshnessValidator::new(1000, 0.1);
        let now = Utc::now();
        
        assert!(validator.validate_market_data_age(now).is_ok());
    }

    #[test]
    fn test_stale_data_fails() {
        let validator = DataFreshnessValidator::new(1000, 0.1);
        let old_time = Utc::now() - Duration::milliseconds(2000);
        
        assert!(validator.validate_market_data_age(old_time).is_err());
    }

    #[test]
    fn test_price_deviation_validation() {
        let validator = DataFreshnessValidator::new(1000, 0.05); // 5% max deviation
        
        // Small deviation should pass
        assert!(validator.validate_price_deviation(100.0, 102.0).is_ok());
        
        // Large deviation should fail
        assert!(validator.validate_price_deviation(100.0, 120.0).is_err());
    }

    #[test]
    fn test_invalid_price_fails() {
        let validator = DataFreshnessValidator::new(1000, 0.1);
        let now = Utc::now();
        
        // Negative price
        assert!(validator.validate_price_data(-10.0, now, None).is_err());
        
        // Zero price
        assert!(validator.validate_price_data(0.0, now, None).is_err());
        
        // NaN price
        assert!(validator.validate_price_data(f64::NAN, now, None).is_err());
    }
}
