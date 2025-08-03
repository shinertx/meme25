use anyhow::Result;
use shared_models::{MarketEvent, StrategyAction, OrderDetails};
use std::collections::HashMap;
use crate::strategies::Strategy;
use crate::database::Database;
use crate::risk_manager::RiskManager;
use tokio::sync::RwLock;
use std::sync::Arc;

pub struct TradingExecutor {
    strategies: HashMap<String, Box<dyn Strategy + Send + Sync>>,
    active_positions: Arc<RwLock<HashMap<String, f64>>>,
    paper_trading: bool,
}

impl TradingExecutor {
    pub fn new(paper_trading: bool) -> Self {
        Self {
            strategies: HashMap::new(),
            active_positions: Arc::new(RwLock::new(HashMap::new())),
            paper_trading,
        }
    }

    pub fn add_strategy(&mut self, name: String, strategy: Box<dyn Strategy + Send + Sync>) {
        self.strategies.insert(name, strategy);
    }

    pub async fn process_event(&mut self, event: &MarketEvent) -> Result<Vec<StrategyAction>> {
        let mut actions = Vec::new();
        
        for (strategy_name, strategy) in &mut self.strategies {
            if strategy.subscriptions().contains(&event.get_type()) {
                match strategy.on_event(event).await {
                    Ok(action) => {
                        if !matches!(action, StrategyAction::Hold) {
                            tracing::info!("Strategy {} generated action: {:?}", strategy_name, action);
                            actions.push(action);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Strategy {} error: {}", strategy_name, e);
                    }
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
                tracing::info!("Reducing position by {}%", percentage * 100.0);
                // TODO: Implement position reduction logic
            }
            StrategyAction::ClosePosition => {
                tracing::info!("Closing all positions");
                // TODO: Implement position closing logic
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
        
        // Update paper positions
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

    async fn live_trade(&self, side: &str, details: &OrderDetails) -> Result<()> {
        tracing::warn!("🚨 LIVE TRADING NOT IMPLEMENTED - would execute: {} {} USD in {}", 
                       side, details.suggested_size_usd, details.symbol);
        // TODO: Implement actual Solana/Jupiter trading
        Ok(())
    }

    pub async fn get_portfolio_summary(&self) -> HashMap<String, f64> {
        self.active_positions.read().await.clone()
    }
}

pub struct MasterExecutor {
    db: Arc<Database>,
    risk_manager: Arc<RiskManager>,
    trading_executor: TradingExecutor,
}

impl MasterExecutor {
    pub async fn new(db: Arc<Database>, risk_manager: Arc<RiskManager>) -> Result<Self> {
        let trading_executor = TradingExecutor::new(true); // Start in paper trading mode
        
        Ok(Self {
            db,
            risk_manager,
            trading_executor,
        })
    }
    
    pub async fn run(&mut self) -> Result<()> {
        tracing::info!("🚀 MasterExecutor starting main execution loop");
        
        // TODO: Add actual event processing loop
        // This is a placeholder for the main execution logic
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}
