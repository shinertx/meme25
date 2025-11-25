use anyhow::Result;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

// Import from the crate's autonomous_coder module
use crate::autonomous_coder::{AutonomousCoder, BacktestMetrics, CodeGenResult, StrategyTemplate};

pub struct AutonomousEvolutionEngine {
    coder: AutonomousCoder,
    generation: u32,
    population_size: usize,
    mutation_rate: f64,
}

impl AutonomousEvolutionEngine {
    pub fn new(paper_trading_mode: bool) -> Self {
        Self {
            coder: AutonomousCoder::new(paper_trading_mode),
            generation: 0,
            population_size: 10,
            mutation_rate: 0.15,
        }
    }

    /// Main autonomous evolution loop
    pub async fn run_evolution_cycle(&mut self) -> Result<()> {
        info!(
            "Starting autonomous evolution cycle, generation {}",
            self.generation
        );

        // Generate initial population or evolve existing
        let templates = if self.generation == 0 {
            self.generate_initial_population()
        } else {
            self.evolve_population().await?
        };

        let mut results = Vec::new();

        // Generate and test each strategy
        for template in templates {
            info!("Generating strategy: {}", template.name);

            match self.coder.generate_strategy(template.clone()).await {
                Ok(result) => {
                    // Auto-commit if profitable
                    if let Err(e) = self.coder.auto_commit(&result).await {
                        warn!("Failed to auto-commit strategy: {}", e);
                    }
                    results.push((template, result));
                }
                Err(e) => {
                    warn!("Failed to generate strategy {}: {}", template.name, e);
                }
            }

            // Brief pause between generations
            sleep(Duration::from_secs(2)).await;
        }

        // Select best performers for next generation
        self.select_survivors(&results);
        self.generation += 1;

        info!(
            "Evolution cycle {} complete, {} strategies generated",
            self.generation,
            results.len()
        );

        Ok(())
    }

    fn generate_initial_population(&self) -> Vec<StrategyTemplate> {
        let base_strategies = vec![
            (
                "momentum_scalp",
                "1m",
                vec!["EMA", "RSI"],
                vec!["price_momentum_up", "volume_spike"],
            ),
            (
                "mean_revert",
                "5m",
                vec!["SMA", "Bollinger"],
                vec!["price_momentum_down", "high_liquidity"],
            ),
            (
                "breakout",
                "15m",
                vec!["ATR", "Volume"],
                vec!["volume_spike", "price_momentum_up"],
            ),
            (
                "dip_buy",
                "5m",
                vec!["RSI", "MACD"],
                vec!["price_momentum_down"],
            ),
        ];

        base_strategies
            .into_iter()
            .enumerate()
            .map(|(i, (name, timeframe, indicators, conditions))| {
                let mut risk_params = HashMap::new();
                risk_params.insert("position_size_pct".to_string(), 2.0 + (i as f64 * 0.5));
                risk_params.insert("stop_loss_pct".to_string(), 3.0 + (i as f64 * 1.0));
                risk_params.insert("take_profit_pct".to_string(), 8.0 + (i as f64 * 2.0));

                StrategyTemplate {
                    name: format!("{}_{}", name, i),
                    timeframe: timeframe.to_string(),
                    indicators: indicators.into_iter().map(|s| s.to_string()).collect(),
                    entry_conditions: conditions.into_iter().map(|s| s.to_string()).collect(),
                    exit_conditions: vec!["trailing_stop".to_string()],
                    risk_params,
                }
            })
            .collect()
    }

    async fn evolve_population(&self) -> Result<Vec<StrategyTemplate>> {
        // For now, generate new variations
        // In full implementation, this would use genetic operators
        Ok(self.generate_mutations())
    }

    fn generate_mutations(&self) -> Vec<StrategyTemplate> {
        let timeframes = vec!["1m", "3m", "5m", "15m"];
        let indicators = vec!["EMA", "SMA", "RSI", "MACD", "ATR", "Bollinger"];
        let conditions = vec![
            "volume_spike",
            "price_momentum_up",
            "price_momentum_down",
            "high_liquidity",
        ];

        (0..self.population_size)
            .map(|i| {
                let mut risk_params = HashMap::new();
                risk_params.insert(
                    "position_size_pct".to_string(),
                    1.0 + (rand::random::<f64>() * 3.0),
                );
                risk_params.insert(
                    "stop_loss_pct".to_string(),
                    2.0 + (rand::random::<f64>() * 8.0),
                );
                risk_params.insert(
                    "take_profit_pct".to_string(),
                    5.0 + (rand::random::<f64>() * 15.0),
                );

                StrategyTemplate {
                    name: format!("evolved_gen{}_{}", self.generation, i),
                    timeframe: timeframes[rand::random::<usize>() % timeframes.len()].to_string(),
                    indicators: indicators
                        .iter()
                        .filter(|_| rand::random::<f64>() < 0.6)
                        .map(|s| s.to_string())
                        .collect(),
                    entry_conditions: conditions
                        .iter()
                        .filter(|_| rand::random::<f64>() < 0.4)
                        .map(|s| s.to_string())
                        .collect(),
                    exit_conditions: vec!["trailing_stop".to_string()],
                    risk_params,
                }
            })
            .collect()
    }

    fn select_survivors(
        &self,
        results: &[(StrategyTemplate, CodeGenResult)],
    ) {
        let mut scored: Vec<_> = results
            .iter()
            .filter_map(|(template, result)| {
                result.backtest_metrics.as_ref().map(|metrics| {
                    let fitness = self.calculate_fitness(metrics);
                    (template, result, fitness)
                })
            })
            .collect();

        scored.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        info!("Top performers this generation:");
        for (i, (template, _result, fitness)) in scored.iter().take(3).enumerate() {
            info!("  {}: {} (fitness: {:.3})", i + 1, template.name, fitness);
        }
    }

    fn calculate_fitness(&self, metrics: &BacktestMetrics) -> f64 {
        // Composite fitness score balancing return and risk
        let sharpe_weight = 0.4;
        let return_weight = 0.3;
        let drawdown_weight = 0.2;
        let winrate_weight = 0.1;

        (metrics.sharpe_ratio * sharpe_weight)
            + (metrics.total_return / 100.0 * return_weight)
            + ((10.0 + metrics.max_drawdown) / 10.0 * drawdown_weight)
            + (metrics.win_rate / 100.0 * winrate_weight)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let paper_mode =
        std::env::var("PAPER_TRADING_MODE").unwrap_or_else(|_| "true".to_string()) == "true";

    let mut engine = AutonomousEvolutionEngine::new(paper_mode);

    info!("🧬 Starting Autonomous Strategy Evolution Engine");
    info!("📊 Paper Trading Mode: {}", paper_mode);

    // Run continuous evolution
    loop {
        if let Err(e) = engine.run_evolution_cycle().await {
            warn!("Evolution cycle failed: {}", e);
        }

        // Sleep between evolution cycles
        let cycle_interval = std::env::var("EVOLUTION_CYCLE_MINUTES")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(60);

        info!(
            "💤 Sleeping {} minutes until next evolution cycle",
            cycle_interval
        );
        sleep(Duration::from_secs(cycle_interval * 60)).await;
    }
}
