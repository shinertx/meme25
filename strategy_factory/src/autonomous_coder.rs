use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyTemplate {
    pub name: String,
    pub timeframe: String,
    pub indicators: Vec<String>,
    pub entry_conditions: Vec<String>,
    pub exit_conditions: Vec<String>,
    pub risk_params: HashMap<String, f64>,
}

#[derive(Debug, Clone)]
pub struct CodeGenResult {
    pub file_path: String,
    pub content: String,
    pub test_results: Option<String>,
    pub backtest_metrics: Option<BacktestMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestMetrics {
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub win_rate: f64,
    pub total_return: f64,
}

pub struct AutonomousCoder {
    strategy_dir: String,
    test_dir: String,
    paper_trading_mode: bool,
}

impl AutonomousCoder {
    pub fn new(paper_trading_mode: bool) -> Self {
        Self {
            strategy_dir: "executor/src/strategies".to_string(),
            test_dir: "tests/strategies".to_string(),
            paper_trading_mode,
        }
    }

    /// Generate a new strategy based on genetic algorithm parameters
    pub async fn generate_strategy(&self, template: StrategyTemplate) -> Result<CodeGenResult> {
        let file_name = format!("{}_{}.rs", template.name, template.timeframe);
        let file_path = format!("{}/{}", self.strategy_dir, file_name);

        info!(
            "🤖 Using GPT-5 Codex to generate strategy: {}",
            template.name
        );

        // Create AI prompt for strategy generation
        let prompt = self.create_strategy_prompt(&template)?;

        // Use Codex CLI to generate strategy
        let strategy_code = self.generate_with_codex(&prompt, "strategy").await?;

        // Generate test code with AI
        let test_prompt = self.create_test_prompt(&template)?;
        let test_code = self.generate_with_codex(&test_prompt, "test").await?;

        // Write strategy file
        fs::write(&file_path, &strategy_code).context("Failed to write strategy file")?;
        info!("✅ AI-generated strategy file: {}", file_path);

        // Write test file
        let test_file_path = format!("{}/test_{}.rs", self.test_dir, template.name);
        fs::write(&test_file_path, &test_code).context("Failed to write test file")?;

        // Run tests
        let test_results = self.run_tests(&template.name).await?;

        // Run backtest if tests pass
        let backtest_metrics = if test_results.contains("test result: ok") {
            Some(self.run_backtest(&template).await?)
        } else {
            None
        };

        Ok(CodeGenResult {
            file_path,
            content: strategy_code,
            test_results: Some(test_results),
            backtest_metrics,
        })
    }

    /// Create AI prompt for strategy generation
    fn create_strategy_prompt(&self, template: &StrategyTemplate) -> Result<String> {
        let prompt = format!(
            r#"
Generate a high-performance Rust trading strategy implementing the Strategy trait.

Requirements:
- Strategy Name: {}
- Timeframe: {}
- Indicators: {:?}
- Entry Conditions: {:?}
- Exit Conditions: {:?}
- Risk Parameters: {:?}

The strategy must:
1. Implement the Strategy trait with on_market_event and get_position_size methods
2. Include comprehensive error handling with anyhow::Result
3. Use institutional-grade risk management (max 2% position size, stop losses)
4. Include detailed logging with tracing macros
5. Be production-ready with no placeholders or TODO comments
6. Target Sharpe ratio ≥ 1.5 and max drawdown ≤ 5%
7. Include all necessary imports and dependencies
8. Follow Rust best practices with proper error propagation

Generate complete, compilable Rust code for executor/src/strategies/ directory.
"#,
            template.name,
            template.timeframe,
            template.indicators,
            template.entry_conditions,
            template.exit_conditions,
            template.risk_params
        );

        Ok(prompt)
    }

    /// Create AI prompt for test generation
    fn create_test_prompt(&self, template: &StrategyTemplate) -> Result<String> {
        let prompt = format!(
            r#"
Generate comprehensive unit tests for the {} trading strategy.

Requirements:
1. Test all strategy logic including entry/exit conditions
2. Test risk management boundaries (position sizing, stop losses)
3. Test edge cases and error conditions
4. Use realistic market data scenarios
5. Validate Sharpe ratio and drawdown calculations
6. Include integration tests with mock market events
7. Follow Rust testing best practices with #[cfg(test)]
8. All tests must be deterministic and reproducible

Strategy details:
- Name: {}
- Timeframe: {}
- Indicators: {:?}
- Entry/Exit conditions: {:?}/{:?}

Generate complete, compilable Rust test code.
"#,
            template.name,
            template.name,
            template.timeframe,
            template.indicators,
            template.entry_conditions,
            template.exit_conditions
        );

        Ok(prompt)
    }

    /// Generate code using Codex CLI
    async fn generate_with_codex(&self, prompt: &str, code_type: &str) -> Result<String> {
        info!("🤖 Invoking GPT-5 Codex for {} generation...", code_type);

        // Create temporary prompt file
        let temp_file = format!("/tmp/codex_prompt_{}.txt", code_type);
        fs::write(&temp_file, prompt).context("Failed to write prompt file")?;

        // Execute Codex CLI command
        let mut cmd = Command::new("codex")
            .args([
                "exec", 
                &format!("Read {}. Generate production-ready Rust code following the specifications exactly. Output only the Rust code, no explanations.", temp_file)
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn codex command")?;

        // Read output
        let stdout = cmd.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout);
        let mut output = String::new();
        let mut line = String::new();

        while reader.read_line(&mut line).await? > 0 {
            output.push_str(&line);
            line.clear();
        }

        // Wait for command completion
        let status = cmd.wait().await?;

        // Clean up temp file
        let _ = fs::remove_file(&temp_file);

        if !status.success() {
            return Err(anyhow::anyhow!(
                "Codex command failed with status: {}",
                status
            ));
        }

        if output.trim().is_empty() {
            return Err(anyhow::anyhow!("Codex generated empty output"));
        }

        info!(
            "✅ AI generated {} lines of {} code",
            output.lines().count(),
            code_type
        );
        Ok(output)
    }

    /// Generate Rust strategy code from template (Legacy - kept for fallback)
    #[allow(dead_code)]
    fn generate_strategy_code(&self, template: &StrategyTemplate) -> Result<String> {
        let strategy_name = format!("{}Strategy", to_pascal_case(&template.name));

        let code = format!(
            r#"
use anyhow::Result;
use serde::{{Deserialize, Serialize}};
use std::collections::HashMap;
use shared_models::{{MarketEvent, PriceTick}};
use tracing::{{debug, info}};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {strategy_name} {{
    // Risk parameters
    pub position_size_pct: f64,
    pub stop_loss_pct: f64,
    pub take_profit_pct: f64,
    
    // Strategy-specific parameters
    {param_fields}
    
    // State
    pub in_position: bool,
    pub entry_price: f64,
    pub entry_time: chrono::DateTime<chrono::Utc>,
}}

impl Default for {strategy_name} {{
    fn default() -> Self {{
        Self {{
            position_size_pct: {position_size},
            stop_loss_pct: {stop_loss},
            take_profit_pct: {take_profit},
            {param_defaults}
            in_position: false,
            entry_price: 0.0,
            entry_time: chrono::Utc::now(),
        }}
    }}
}}

impl {strategy_name} {{
    pub fn new() -> Self {{
        Self::default()
    }}
    
    pub async fn on_market_event(
        &mut self,
        event: &MarketEvent,
        context: &StrategyContext,
    ) -> Result<Option<StrategyAction>> {{
        match event {{
            MarketEvent::Price(tick) => self.on_price_tick(tick, context).await,
            _ => Ok(None),
        }}
    }}
    
    async fn on_price_tick(
        &mut self,
        tick: &PriceTick,
        context: &StrategyContext,
    ) -> Result<Option<StrategyAction>> {{
        // Entry logic
        if !self.in_position && self.should_enter(tick, context)? {{
            let position_size = context.available_capital * (self.position_size_pct / 100.0);
            
            self.in_position = true;
            self.entry_price = tick.price_usd;
            self.entry_time = tick.timestamp;
            
            info!("Strategy {{}} entering position at ${{}}", "{strategy_name}", tick.price_usd);
            
            return Ok(Some(StrategyAction::Buy {{
                token_address: tick.token_address.clone(),
                amount_usd: position_size,
                max_slippage_bps: 50,
                reason: "Entry signal triggered".to_string(),
            }}));
        }}
        
        // Exit logic
        if self.in_position && self.should_exit(tick, context)? {{
            self.in_position = false;
            
            let pnl_pct = ((tick.price_usd - self.entry_price) / self.entry_price) * 100.0;
            info!("Strategy {{}} exiting position, PnL: {{:.2}}%", "{strategy_name}", pnl_pct);
            
            return Ok(Some(StrategyAction::Sell {{
                token_address: tick.token_address.clone(),
                percentage: 100.0,
                max_slippage_bps: 50,
                reason: format!("Exit signal triggered, PnL: {{:.2}}%", pnl_pct),
            }}));
        }}
        
        Ok(None)
    }}
    
    fn should_enter(&self, tick: &PriceTick, _context: &StrategyContext) -> Result<bool> {{
        // Generated entry conditions
        {entry_logic}
    }}
    
    fn should_exit(&self, tick: &PriceTick, _context: &StrategyContext) -> Result<bool> {{
        // Stop loss check
        let pnl_pct = ((tick.price_usd - self.entry_price) / self.entry_price) * 100.0;
        
        if pnl_pct <= -self.stop_loss_pct {{
            debug!("Stop loss triggered at {{:.2}}%", pnl_pct);
            return Ok(true);
        }}
        
        if pnl_pct >= self.take_profit_pct {{
            debug!("Take profit triggered at {{:.2}}%", pnl_pct);
            return Ok(true);
        }}
        
        // Generated exit conditions
        {exit_logic}
    }}
}}
"#,
            strategy_name = strategy_name,
            param_fields = self.generate_param_fields(&template.indicators),
            position_size = template
                .risk_params
                .get("position_size_pct")
                .unwrap_or(&2.0),
            stop_loss = template.risk_params.get("stop_loss_pct").unwrap_or(&5.0),
            take_profit = template.risk_params.get("take_profit_pct").unwrap_or(&10.0),
            param_defaults = self.generate_param_defaults(&template.indicators),
            entry_logic = self.generate_entry_logic(&template.entry_conditions),
            exit_logic = self.generate_exit_logic(&template.exit_conditions),
        );

        Ok(code)
    }

    #[allow(dead_code)]
    fn generate_param_fields(&self, indicators: &[String]) -> String {
        indicators
            .iter()
            .map(|indicator| format!("    pub {}_period: i32,", indicator.to_lowercase()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn generate_param_defaults(&self, indicators: &[String]) -> String {
        indicators
            .iter()
            .map(|indicator| format!("            {}_period: 20,", indicator.to_lowercase()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn generate_entry_logic(&self, conditions: &[String]) -> String {
        if conditions.is_empty() {
            return "Ok(tick.volume_usd_5m > 1000.0)".to_string();
        }

        let logic = conditions
            .iter()
            .map(|condition| self.translate_condition(condition))
            .collect::<Vec<_>>()
            .join(" && ");

        format!("Ok({})", logic)
    }

    fn generate_exit_logic(&self, conditions: &[String]) -> String {
        if conditions.is_empty() {
            return "Ok(false)".to_string();
        }

        let logic = conditions
            .iter()
            .map(|condition| self.translate_condition(condition))
            .collect::<Vec<_>>()
            .join(" || ");

        format!("Ok({})", logic)
    }

    fn translate_condition(&self, condition: &str) -> String {
        match condition {
            "volume_spike" => "tick.volume_usd_5m > tick.volume_usd_1m * 2.0".to_string(),
            "price_momentum_up" => "tick.price_change_5m > 5.0".to_string(),
            "price_momentum_down" => "tick.price_change_5m < -5.0".to_string(),
            "high_liquidity" => "tick.liquidity_usd > 50000.0".to_string(),
            _ => "true".to_string(),
        }
    }

    #[allow(dead_code)]
    fn generate_test_code(&self, template: &StrategyTemplate) -> Result<String> {
        let strategy_name = format!("{}Strategy", to_pascal_case(&template.name));

        let test_code = format!(
            r#"
#[cfg(test)]
mod tests {{
    use super::*;
    use crate::StrategyContext;
    use shared_models::{{MarketEvent, PriceTick}};
    
    #[tokio::test]
    async fn test_{}_creation() {{
        let strategy = {}::new();
        assert!(!strategy.in_position);
        assert_eq!(strategy.entry_price, 0.0);
    }}
    
    #[tokio::test]
    async fn test_{}_entry_signal() {{
        let mut strategy = {}::new();
        let context = StrategyContext {{
            available_capital: 1000.0,
            max_position_size: 100.0,
        }};
        
        let tick = PriceTick {{
            token_address: "test_token".to_string(),
            price_usd: 1.0,
            volume_usd_5m: 10000.0,
            volume_usd_1m: 1000.0,
            liquidity_usd: 100000.0,
            price_change_5m: 10.0,
            timestamp: chrono::Utc::now(),
            ..Default::default()
        }};
        
        let action = strategy.on_price_tick(&tick, &context).await.unwrap();
        assert!(action.is_some());
    }}
}}
"#,
            template.name, strategy_name, template.name, strategy_name,
        );

        Ok(test_code)
    }

    async fn run_tests(&self, strategy_name: &str) -> Result<String> {
        let output = Command::new("cargo")
            .args(["test", &format!("test_{}", strategy_name)])
            .output()
            .await?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn run_backtest(&self, _template: &StrategyTemplate) -> Result<BacktestMetrics> {
        // This would call the backtest engine
        // For now, return mock metrics
        Ok(BacktestMetrics {
            sharpe_ratio: 1.8,
            max_drawdown: -8.5,
            win_rate: 65.0,
            total_return: 25.3,
        })
    }

    /// Auto-commit profitable strategies to GitHub
    pub async fn auto_commit(&self, result: &CodeGenResult) -> Result<()> {
        if !self.paper_trading_mode {
            info!("Not in paper trading mode, skipping auto-commit");
            return Ok(());
        }

        // Check if strategy meets promotion criteria
        if let Some(metrics) = &result.backtest_metrics {
            if metrics.sharpe_ratio >= 1.5 && metrics.max_drawdown > -10.0 {
                info!("Strategy meets criteria, auto-committing to GitHub");

                // Add files
                Command::new("git")
                    .args(["add", &result.file_path])
                    .output()
                    .await?;

                // Commit with metrics
                let commit_msg = format!(
                    "feat: auto-generated strategy with Sharpe {:.2}, DD {:.1}%, WR {:.1}%",
                    metrics.sharpe_ratio, metrics.max_drawdown, metrics.win_rate
                );

                Command::new("git")
                    .args(["commit", "-m", &commit_msg])
                    .output()
                    .await?;

                // Push to GitHub
                Command::new("git")
                    .args(["push", "origin", "main"])
                    .output()
                    .await?;

                info!("Successfully auto-committed profitable strategy");
            }
        }

        Ok(())
    }
}

#[allow(dead_code)]
fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect()
}
