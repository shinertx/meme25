use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared_models::error::{ModelError, Result};
use std::time::Duration;
use tracing::{debug, info};

/// Jito client for MEV protection and transaction submission
pub struct JitoClient {
    client: Client,
    block_engine_url: String,
    auth_keypair: Option<String>, // Path to Jito auth keypair
    tip_lamports: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JitoBundle {
    pub transactions: Vec<String>, // Base64 encoded transactions
    pub tip_account: String,
    pub tip_lamports: u64,
}

#[derive(Debug, Deserialize)]
pub struct JitoBundleResponse {
    pub bundle_id: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct JitoTipAccounts {
    pub tip_accounts: Vec<String>,
}

impl JitoClient {
    pub fn new(
        block_engine_url: String,
        auth_keypair: Option<String>,
        tip_lamports: u64,
    ) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| ModelError::Network(format!("Failed to create Jito client: {}", e)))?;

        Ok(Self {
            client,
            block_engine_url,
            auth_keypair,
            tip_lamports,
        })
    }

    /// Get current tip accounts from Jito
    pub async fn get_tip_accounts(&self) -> Result<Vec<String>> {
        let url = format!("{}/api/v1/bundles/tip_accounts", self.block_engine_url);

        debug!("Fetching Jito tip accounts from: {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ModelError::Network(format!("Failed to get tip accounts: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(ModelError::Network(format!(
                "Jito tip accounts request failed: {} - {}",
                status, error_text
            )));
        }

        let tip_data: JitoTipAccounts = response.json().await.map_err(|e| {
            ModelError::Network(format!("Failed to parse tip accounts response: {}", e))
        })?;

        info!(
            "Retrieved {} Jito tip accounts",
            tip_data.tip_accounts.len()
        );
        Ok(tip_data.tip_accounts)
    }

    /// Submit a transaction bundle to Jito for MEV protection
    pub async fn submit_bundle(&self, transactions: Vec<String>) -> Result<String> {
        let tip_accounts = self.get_tip_accounts().await?;

        if tip_accounts.is_empty() {
            return Err(ModelError::Network("No Jito tip accounts available".into()));
        }

        // Use the first tip account
        let tip_account = &tip_accounts[0];

        let bundle = JitoBundle {
            transactions,
            tip_account: tip_account.clone(),
            tip_lamports: self.tip_lamports,
        };

        let url = format!("{}/api/v1/bundles", self.block_engine_url);

        debug!(
            "Submitting bundle to Jito: {} transactions, tip: {} lamports",
            bundle.transactions.len(),
            bundle.tip_lamports
        );

        let response = self
            .client
            .post(&url)
            .json(&bundle)
            .send()
            .await
            .map_err(|e| ModelError::Network(format!("Failed to submit bundle: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(ModelError::Network(format!(
                "Jito bundle submission failed: {} - {}",
                status, error_text
            )));
        }

        let bundle_response: JitoBundleResponse = response
            .json()
            .await
            .map_err(|e| ModelError::Network(format!("Failed to parse bundle response: {}", e)))?;

        info!(
            "Bundle submitted successfully: {}",
            bundle_response.bundle_id
        );
        Ok(bundle_response.bundle_id)
    }

    /// Check the status of a submitted bundle
    pub async fn get_bundle_status(&self, bundle_id: &str) -> Result<String> {
        let url = format!("{}/api/v1/bundles/{}", self.block_engine_url, bundle_id);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ModelError::Network(format!("Failed to get bundle status: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(ModelError::Network(format!(
                "Bundle status request failed: {} - {}",
                status, error_text
            )));
        }

        let status_response: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ModelError::Network(format!("Failed to parse status response: {}", e)))?;

        let status = status_response
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("unknown")
            .to_string();

        debug!("Bundle {} status: {}", bundle_id, status);
        Ok(status)
    }

    /// Create a MEV-protected transaction submission
    pub async fn submit_protected_transaction(&self, transaction_b64: String) -> Result<String> {
        // For single transactions, we still use bundles for MEV protection
        let transactions = vec![transaction_b64];
        self.submit_bundle(transactions).await
    }

    /// Estimate optimal tip based on current network conditions
    pub async fn estimate_tip(&self) -> Result<u64> {
        // For now, return the configured tip
        // In a production system, this would analyze current MEV activity
        // and suggest optimal tips based on transaction priority
        Ok(self.tip_lamports)
    }
}

/// MEV Protection Strategy
pub enum MevProtectionLevel {
    None,          // Standard RPC submission
    Basic,         // Jito bundle with standard tip
    Aggressive,    // Higher tip for high-priority transactions
    MaxProtection, // Maximum tip for critical trades
}

impl MevProtectionLevel {
    pub fn get_tip_multiplier(&self) -> f64 {
        match self {
            MevProtectionLevel::None => 0.0,
            MevProtectionLevel::Basic => 1.0,
            MevProtectionLevel::Aggressive => 2.0,
            MevProtectionLevel::MaxProtection => 5.0,
        }
    }
}

/// MEV Protection Manager
pub struct MevProtectionManager {
    jito_client: JitoClient,
    base_tip: u64,
}

impl MevProtectionManager {
    pub fn new(jito_client: JitoClient, base_tip: u64) -> Self {
        Self {
            jito_client,
            base_tip,
        }
    }

    /// Submit transaction with appropriate MEV protection
    pub async fn submit_with_protection(
        &self,
        transaction_b64: String,
        protection_level: MevProtectionLevel,
    ) -> Result<String> {
        match protection_level {
            MevProtectionLevel::None => {
                // Submit directly to RPC without MEV protection
                // This would integrate with standard Solana RPC
                info!("Submitting transaction without MEV protection");
                Ok("direct_submission".to_string()) // Placeholder
            }
            _ => {
                // Calculate tip based on protection level
                let tip_multiplier = protection_level.get_tip_multiplier();
                let adjusted_tip = (self.base_tip as f64 * tip_multiplier) as u64;

                info!(
                    "Submitting transaction with MEV protection, tip: {} lamports",
                    adjusted_tip
                );

                // Create a new Jito client with the adjusted tip
                let protected_client = JitoClient::new(
                    self.jito_client.block_engine_url.clone(),
                    self.jito_client.auth_keypair.clone(),
                    adjusted_tip,
                )?;

                protected_client
                    .submit_protected_transaction(transaction_b64)
                    .await
            }
        }
    }

    /// Determine optimal protection level based on trade characteristics
    pub fn determine_protection_level(
        &self,
        trade_size_usd: f64,
        is_arbitrage: bool,
        market_volatility: f64,
    ) -> MevProtectionLevel {
        // Large trades or arbitrage opportunities need more protection
        if trade_size_usd > 1000.0 || is_arbitrage {
            MevProtectionLevel::MaxProtection
        } else if trade_size_usd > 500.0 || market_volatility > 0.1 {
            MevProtectionLevel::Aggressive
        } else if trade_size_usd > 100.0 {
            MevProtectionLevel::Basic
        } else {
            MevProtectionLevel::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protection_level_tips() {
        assert_eq!(MevProtectionLevel::None.get_tip_multiplier(), 0.0);
        assert_eq!(MevProtectionLevel::Basic.get_tip_multiplier(), 1.0);
        assert_eq!(MevProtectionLevel::Aggressive.get_tip_multiplier(), 2.0);
        assert_eq!(MevProtectionLevel::MaxProtection.get_tip_multiplier(), 5.0);
    }

    #[test]
    fn test_protection_level_determination() {
        let jito_client = JitoClient::new(
            "https://mainnet.block-engine.jito.wtf".to_string(),
            None,
            10000,
        )
        .unwrap();

        let manager = MevProtectionManager::new(jito_client, 10000);

        // Large trade should get max protection
        let level = manager.determine_protection_level(1500.0, false, 0.05);
        assert!(matches!(level, MevProtectionLevel::MaxProtection));

        // Arbitrage should get max protection
        let level = manager.determine_protection_level(200.0, true, 0.02);
        assert!(matches!(level, MevProtectionLevel::MaxProtection));

        // Medium trade should get aggressive protection
        let level = manager.determine_protection_level(600.0, false, 0.03);
        assert!(matches!(level, MevProtectionLevel::Aggressive));

        // Small trade should get basic protection
        let level = manager.determine_protection_level(150.0, false, 0.02);
        assert!(matches!(level, MevProtectionLevel::Basic));

        // Very small trade should get no protection
        let level = manager.determine_protection_level(50.0, false, 0.01);
        assert!(matches!(level, MevProtectionLevel::None));
    }
}
