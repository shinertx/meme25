use chrono::{DateTime, Duration, Utc};
use rand::{seq::SliceRandom, thread_rng, Rng};
use serde::{Deserialize, Serialize};
use shared_models::error::Result;
use shared_models::{StrategyPerformance, StrategySpec};
use std::collections::{HashMap, VecDeque};
use tracing::{debug, info};

/// Automated strategy evolution using genetic algorithms and machine learning
#[derive(Debug)]
pub struct StrategyEvolution {
    // Population management
    current_population: Vec<EvolutionCandidate>,
    population_size: usize,
    elite_size: usize,

    // Performance tracking
    performance_history: HashMap<String, Vec<PerformanceRecord>>,
    fitness_scores: HashMap<String, f64>,

    // Evolution parameters
    mutation_rate: f64,
    crossover_rate: f64,
    _selection_pressure: f64,

    // Machine learning components
    _feature_extractors: Vec<FeatureExtractor>,
    _performance_predictors: HashMap<String, PerformancePredictor>,

    // Strategy lifecycle
    strategy_generations: HashMap<String, u32>,
    _survival_threshold: f64,
    _max_generations: u32,

    // Evolution history
    evolution_history: VecDeque<EvolutionEpoch>,
    last_evolution: DateTime<Utc>,
    evolution_frequency: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionCandidate {
    pub strategy_id: String,
    pub strategy_type: String,
    pub parameters: serde_json::Value,
    pub fitness_score: f64,
    pub age_generations: u32,
    pub parent_ids: Vec<String>,
    pub creation_time: DateTime<Utc>,
    pub last_update: DateTime<Utc>,
    pub performance_metrics: CandidateMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateMetrics {
    pub sharpe_ratio: f64,
    pub total_return: f64,
    pub max_drawdown: f64,
    pub win_rate: f64,
    pub trade_count: u32,
    pub volatility: f64,
    pub calmar_ratio: f64,
    pub sortino_ratio: f64,
    pub stability_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceRecord {
    pub timestamp: DateTime<Utc>,
    pub strategy_id: String,
    pub pnl: f64,
    pub trade_count: u32,
    pub sharpe_ratio: f64,
    pub drawdown: f64,
    pub market_conditions: MarketConditions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketConditions {
    pub volatility_regime: VolatilityRegime,
    pub trend_direction: TrendDirection,
    pub liquidity_level: LiquidityLevel,
    pub correlation_environment: CorrelationEnvironment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VolatilityRegime {
    Low,
    Medium,
    High,
    Extreme,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    StrongUp,
    Up,
    Sideways,
    Down,
    StrongDown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LiquidityLevel {
    VeryHigh,
    High,
    Medium,
    Low,
    VeryLow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CorrelationEnvironment {
    LowCorrelation,
    MediumCorrelation,
    HighCorrelation,
    Crisis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureExtractor {
    pub name: String,
    pub feature_type: FeatureType,
    pub lookback_period: Duration,
    pub normalization_method: NormalizationMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeatureType {
    Price,
    Volume,
    Volatility,
    Momentum,
    MeanReversion,
    Liquidity,
    Social,
    OnChain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NormalizationMethod {
    ZScore,
    MinMax,
    Robust,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformancePredictor {
    pub model_type: ModelType,
    pub feature_weights: HashMap<String, f64>,
    pub prediction_accuracy: f64,
    pub last_training: DateTime<Utc>,
    pub training_data_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelType {
    LinearRegression,
    RandomForest,
    GradientBoosting,
    NeuralNetwork,
    Ensemble,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionEpoch {
    pub timestamp: DateTime<Utc>,
    pub generation: u32,
    pub population_size: usize,
    pub average_fitness: f64,
    pub best_fitness: f64,
    pub worst_fitness: f64,
    pub fitness_variance: f64,
    pub new_strategies_created: u32,
    pub strategies_eliminated: u32,
    pub market_conditions: MarketConditions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionReport {
    pub timestamp: DateTime<Utc>,
    pub current_generation: u32,
    pub population_stats: PopulationStats,
    pub top_performers: Vec<EvolutionCandidate>,
    pub evolution_metrics: EvolutionMetrics,
    pub recommended_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopulationStats {
    pub total_strategies: usize,
    pub average_age: f64,
    pub strategy_type_distribution: HashMap<String, u32>,
    pub performance_distribution: PerformanceDistribution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceDistribution {
    pub mean_fitness: f64,
    pub median_fitness: f64,
    pub std_fitness: f64,
    pub percentile_95: f64,
    pub percentile_75: f64,
    pub percentile_25: f64,
    pub percentile_5: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionMetrics {
    pub convergence_rate: f64,
    pub diversity_index: f64,
    pub adaptation_speed: f64,
    pub stability_score: f64,
    pub innovation_rate: f64,
}

impl StrategyEvolution {
    pub fn new(population_size: usize) -> Self {
        Self {
            current_population: Vec::with_capacity(population_size),
            population_size,
            elite_size: (population_size as f64 * 0.2) as usize, // Top 20% survive
            performance_history: HashMap::new(),
            fitness_scores: HashMap::new(),
            mutation_rate: 0.15,      // 15% mutation rate
            crossover_rate: 0.7,      // 70% crossover rate
            _selection_pressure: 2.0, // Tournament selection pressure
            _feature_extractors: Self::initialize_feature_extractors(),
            _performance_predictors: HashMap::new(),
            strategy_generations: HashMap::new(),
            _survival_threshold: 0.1, // Bottom 10% eliminated
            _max_generations: 100,
            evolution_history: VecDeque::with_capacity(1000),
            last_evolution: Utc::now() - Duration::hours(24), // Force first evolution
            evolution_frequency: Duration::hours(6),          // Evolve every 6 hours
        }
    }

    /// Initialize the population with seed strategies
    pub fn initialize_population(&mut self, seed_strategies: Vec<StrategySpec>) -> Result<()> {
        self.current_population.clear();

        for spec in seed_strategies {
            let candidate = EvolutionCandidate {
                strategy_id: spec.id.clone(),
                strategy_type: spec.family.clone(),
                parameters: spec.params.clone(),
                fitness_score: 0.0,
                age_generations: 0,
                parent_ids: vec![],
                creation_time: Utc::now(),
                last_update: Utc::now(),
                performance_metrics: CandidateMetrics::default(),
            };

            self.current_population.push(candidate);
            self.strategy_generations.insert(spec.id, 0);
        }

        // Fill remaining slots with mutated versions of seeds
        while self.current_population.len() < self.population_size {
            if let Some(base_candidate) = self.current_population.choose(&mut thread_rng()).cloned()
            {
                let mutated = self.mutate_strategy(&base_candidate)?;
                self.current_population.push(mutated);
            }
        }

        info!(
            population_size = self.current_population.len(),
            "Strategy evolution population initialized"
        );

        Ok(())
    }

    /// Update performance data for strategies
    pub fn update_performance(
        &mut self,
        strategy_id: &str,
        performance: &StrategyPerformance,
        market_conditions: MarketConditions,
    ) -> Result<()> {
        // Record performance history
        let record = PerformanceRecord {
            timestamp: Utc::now(),
            strategy_id: strategy_id.to_string(),
            pnl: performance.total_pnl_usd,
            trade_count: performance.total_trades,
            sharpe_ratio: performance.sharpe_ratio,
            drawdown: performance.max_drawdown_pct,
            market_conditions,
        };

        self.performance_history
            .entry(strategy_id.to_string())
            .or_default()
            .push(record);

        // Update candidate metrics if in population - handle borrowing carefully
        let strategy_id_str = strategy_id;

        // Calculate stability score first (no mutable borrow)
        let stability_score = self.calculate_stability_score(strategy_id_str);

        // Create metrics struct
        let new_metrics = CandidateMetrics {
            sharpe_ratio: performance.sharpe_ratio,
            total_return: performance.total_pnl_usd, // Use total PnL as return
            max_drawdown: performance.max_drawdown_pct,
            win_rate: performance.win_rate,
            trade_count: performance.total_trades,
            volatility: 0.0, // Not available in StrategyPerformance
            calmar_ratio: if performance.max_drawdown_pct > 0.0 {
                performance.total_pnl_usd / performance.max_drawdown_pct
            } else {
                0.0
            },
            sortino_ratio: performance.sortino_ratio,
            stability_score,
        };

        // Calculate fitness score (no mutable borrow)
        let fitness_score = self.calculate_fitness_score(&new_metrics);

        // Now update the candidate with mutable borrow
        if let Some(candidate) = self
            .current_population
            .iter_mut()
            .find(|c| c.strategy_id == strategy_id_str)
        {
            candidate.performance_metrics = new_metrics;
            candidate.fitness_score = fitness_score;
            candidate.last_update = Utc::now();
        }

        debug!(
            strategy_id = strategy_id,
            sharpe_ratio = performance.sharpe_ratio,
            total_return = performance.total_pnl_usd,
            "Performance updated for strategy"
        );

        Ok(())
    }

    /// Check if evolution should be triggered
    pub fn should_evolve(&self) -> bool {
        Utc::now().signed_duration_since(self.last_evolution) >= self.evolution_frequency
    }

    /// Perform evolution step
    pub fn evolve(
        &mut self,
        current_market_conditions: MarketConditions,
    ) -> Result<EvolutionReport> {
        info!("Starting strategy evolution process");

        // Calculate fitness scores for all candidates
        self.update_all_fitness_scores();

        // Sort population by fitness (descending)
        self.current_population
            .sort_by(|a, b| b.fitness_score.partial_cmp(&a.fitness_score).unwrap());

        // Record current population stats
        let epoch = self.create_evolution_epoch(&current_market_conditions);
        self.evolution_history.push_back(epoch);

        // Keep only recent history
        if self.evolution_history.len() > 1000 {
            self.evolution_history.pop_front();
        }

        // Selection: Keep elite performers
        let elite_count = self.elite_size.min(self.current_population.len());
        let mut new_population = self.current_population[..elite_count].to_vec();

        // Age elite strategies
        for candidate in &mut new_population {
            candidate.age_generations += 1;
        }

        let mut new_strategies_created = 0;

        // Generate new strategies to fill population
        while new_population.len() < self.population_size {
            let mut rng = thread_rng();

            if rng.gen::<f64>() < self.crossover_rate && new_population.len() >= 2 {
                // Crossover: Create child from two parents
                let parent1 = self.tournament_selection(&new_population)?;
                let parent2 = self.tournament_selection(&new_population)?;

                if parent1.strategy_id != parent2.strategy_id {
                    let child = self.crossover_strategies(&parent1, &parent2)?;
                    new_population.push(child);
                    new_strategies_created += 1;
                }
            } else {
                // Mutation: Create mutated version of existing strategy
                let parent = self.tournament_selection(&new_population)?;
                let mutated = self.mutate_strategy(&parent)?;
                new_population.push(mutated);
                new_strategies_created += 1;
            }
        }

        // Calculate elimination count
        let strategies_eliminated = self.current_population.len() - elite_count;

        // Update population
        self.current_population = new_population;
        self.last_evolution = Utc::now();

        // Generate evolution report
        let report = self.generate_evolution_report(
            new_strategies_created,
            strategies_eliminated as u32,
            &current_market_conditions,
        );

        info!(
            generation = self.get_current_generation(),
            elite_kept = elite_count,
            new_created = new_strategies_created,
            eliminated = strategies_eliminated,
            avg_fitness = self.calculate_average_fitness(),
            "Evolution step completed"
        );

        Ok(report)
    }

    /// Get strategies that should be deployed for live trading
    pub fn get_deployment_candidates(&self, count: usize) -> Vec<EvolutionCandidate> {
        let mut candidates = self.current_population.clone();

        // Sort by fitness score
        candidates.sort_by(|a, b| b.fitness_score.partial_cmp(&a.fitness_score).unwrap());

        // Take top performers with minimum fitness threshold
        candidates
            .into_iter()
            .filter(|c| c.fitness_score > 0.5) // Minimum fitness threshold
            .take(count)
            .collect()
    }

    /// Generate comprehensive evolution report
    pub fn generate_evolution_report(
        &self,
        _new_strategies: u32,
        _eliminated_strategies: u32,
        _market_conditions: &MarketConditions,
    ) -> EvolutionReport {
        let population_stats = self.calculate_population_stats();
        let evolution_metrics = self.calculate_evolution_metrics();
        let top_performers = self.get_deployment_candidates(10);
        let recommended_actions = self.generate_recommendations(&evolution_metrics);

        EvolutionReport {
            timestamp: Utc::now(),
            current_generation: self.get_current_generation(),
            population_stats,
            top_performers,
            evolution_metrics,
            recommended_actions,
        }
    }

    // Private helper methods
    fn initialize_feature_extractors() -> Vec<FeatureExtractor> {
        vec![
            FeatureExtractor {
                name: "price_momentum".to_string(),
                feature_type: FeatureType::Momentum,
                lookback_period: Duration::hours(1),
                normalization_method: NormalizationMethod::ZScore,
            },
            FeatureExtractor {
                name: "volume_profile".to_string(),
                feature_type: FeatureType::Volume,
                lookback_period: Duration::minutes(30),
                normalization_method: NormalizationMethod::MinMax,
            },
            FeatureExtractor {
                name: "volatility_regime".to_string(),
                feature_type: FeatureType::Volatility,
                lookback_period: Duration::hours(4),
                normalization_method: NormalizationMethod::Robust,
            },
        ]
    }

    fn calculate_fitness_score(&self, metrics: &CandidateMetrics) -> f64 {
        // Multi-objective fitness function balancing return, risk, and stability
        let return_component = metrics.total_return * 0.3;
        let risk_adjusted_component = metrics.sharpe_ratio * 0.25;
        let drawdown_penalty = -metrics.max_drawdown * 0.15;
        let win_rate_component = metrics.win_rate * 0.15;
        let stability_component = metrics.stability_score * 0.15;

        (return_component
            + risk_adjusted_component
            + drawdown_penalty
            + win_rate_component
            + stability_component)
            .max(0.0)
    }

    fn calculate_stability_score(&self, strategy_id: &str) -> f64 {
        if let Some(history) = self.performance_history.get(strategy_id) {
            if history.len() < 5 {
                return 0.5; // Default for new strategies
            }

            // Calculate consistency of performance
            let returns: Vec<f64> = history.windows(2).map(|w| w[1].pnl - w[0].pnl).collect();

            if returns.is_empty() {
                return 0.5;
            }

            let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
            let variance = returns
                .iter()
                .map(|r| (r - mean_return).powi(2))
                .sum::<f64>()
                / returns.len() as f64;

            // Higher stability = lower variance relative to mean
            if variance > 0.0 && mean_return.abs() > 0.0 {
                (1.0 / (1.0 + variance / mean_return.abs())).min(1.0)
            } else {
                0.5
            }
        } else {
            0.5
        }
    }

    fn update_all_fitness_scores(&mut self) {
        // Collect fitness scores first to avoid borrowing issues
        let fitness_updates: Vec<(String, f64)> = self
            .current_population
            .iter()
            .map(|candidate| {
                let fitness = self.calculate_fitness_score(&candidate.performance_metrics);
                (candidate.strategy_id.clone(), fitness)
            })
            .collect();

        // Update the candidates with new fitness scores
        for (strategy_id, fitness) in fitness_updates {
            if let Some(candidate) = self
                .current_population
                .iter_mut()
                .find(|c| c.strategy_id == strategy_id)
            {
                candidate.fitness_score = fitness;
            }
            self.fitness_scores.insert(strategy_id, fitness);
        }
    }

    fn tournament_selection(
        &self,
        population: &[EvolutionCandidate],
    ) -> Result<EvolutionCandidate> {
        let tournament_size = (population.len() as f64 * 0.1).max(2.0) as usize;
        let mut rng = thread_rng();

        let mut tournament: Vec<&EvolutionCandidate> = population
            .choose_multiple(&mut rng, tournament_size)
            .collect();

        tournament.sort_by(|a, b| b.fitness_score.partial_cmp(&a.fitness_score).unwrap());

        Ok(tournament[0].clone())
    }

    fn crossover_strategies(
        &self,
        parent1: &EvolutionCandidate,
        parent2: &EvolutionCandidate,
    ) -> Result<EvolutionCandidate> {
        let mut rng = thread_rng();

        // Create child with blended parameters
        let child_id = format!(
            "{}x{}_{}",
            &parent1.strategy_id[..6],
            &parent2.strategy_id[..6],
            rng.gen::<u32>()
        );

        // Blend numeric parameters
        let mut child_params = parent1.parameters.clone();
        if let (Some(p1_obj), Some(p2_obj)) = (
            parent1.parameters.as_object(),
            parent2.parameters.as_object(),
        ) {
            if let Some(child_obj) = child_params.as_object_mut() {
                for (key, value) in p1_obj {
                    if let (Some(v1), Some(v2)) =
                        (value.as_f64(), p2_obj.get(key).and_then(|v| v.as_f64()))
                    {
                        // Blend numeric values
                        let alpha = rng.gen::<f64>();
                        let blended = v1 * alpha + v2 * (1.0 - alpha);
                        child_obj.insert(key.clone(), serde_json::json!(blended));
                    }
                }
            }
        }

        Ok(EvolutionCandidate {
            strategy_id: child_id,
            strategy_type: if rng.gen::<bool>() {
                parent1.strategy_type.clone()
            } else {
                parent2.strategy_type.clone()
            },
            parameters: child_params,
            fitness_score: 0.0,
            age_generations: 0,
            parent_ids: vec![parent1.strategy_id.clone(), parent2.strategy_id.clone()],
            creation_time: Utc::now(),
            last_update: Utc::now(),
            performance_metrics: CandidateMetrics::default(),
        })
    }

    fn mutate_strategy(&self, parent: &EvolutionCandidate) -> Result<EvolutionCandidate> {
        let mut rng = thread_rng();

        let mutated_id = format!("{}m_{}", &parent.strategy_id[..8], rng.gen::<u32>());
        let mut mutated_params = parent.parameters.clone();

        // Mutate parameters
        if let Some(obj) = mutated_params.as_object_mut() {
            for (_key, value) in obj.iter_mut() {
                if rng.gen::<f64>() < self.mutation_rate {
                    if let Some(num_val) = value.as_f64() {
                        // Gaussian mutation for numeric values
                        let mutation_strength = 0.1; // 10% of current value
                        let noise = rng.gen_range(-1.0..1.0) * num_val * mutation_strength;
                        *value = serde_json::json!(num_val + noise);
                    }
                }
            }
        }

        Ok(EvolutionCandidate {
            strategy_id: mutated_id,
            strategy_type: parent.strategy_type.clone(),
            parameters: mutated_params,
            fitness_score: 0.0,
            age_generations: 0,
            parent_ids: vec![parent.strategy_id.clone()],
            creation_time: Utc::now(),
            last_update: Utc::now(),
            performance_metrics: CandidateMetrics::default(),
        })
    }

    fn create_evolution_epoch(&self, market_conditions: &MarketConditions) -> EvolutionEpoch {
        let fitness_scores: Vec<f64> = self
            .current_population
            .iter()
            .map(|c| c.fitness_score)
            .collect();

        let average_fitness = fitness_scores.iter().sum::<f64>() / fitness_scores.len() as f64;
        let best_fitness = fitness_scores.iter().fold(0.0_f64, |a, &b| a.max(b));
        let worst_fitness = fitness_scores.iter().fold(f64::INFINITY, |a, &b| a.min(b));

        let variance = fitness_scores
            .iter()
            .map(|f| (f - average_fitness).powi(2))
            .sum::<f64>()
            / fitness_scores.len() as f64;

        EvolutionEpoch {
            timestamp: Utc::now(),
            generation: self.get_current_generation(),
            population_size: self.current_population.len(),
            average_fitness,
            best_fitness,
            worst_fitness,
            fitness_variance: variance,
            new_strategies_created: 0, // Will be filled by caller
            strategies_eliminated: 0,  // Will be filled by caller
            market_conditions: market_conditions.clone(),
        }
    }

    fn calculate_population_stats(&self) -> PopulationStats {
        let total_age: u32 = self
            .current_population
            .iter()
            .map(|c| c.age_generations)
            .sum();
        let average_age = if !self.current_population.is_empty() {
            total_age as f64 / self.current_population.len() as f64
        } else {
            0.0
        };

        let mut strategy_type_dist = HashMap::new();
        for candidate in &self.current_population {
            *strategy_type_dist
                .entry(candidate.strategy_type.clone())
                .or_insert(0) += 1;
        }

        let fitness_scores: Vec<f64> = self
            .current_population
            .iter()
            .map(|c| c.fitness_score)
            .collect();

        let performance_distribution = self.calculate_performance_distribution(&fitness_scores);

        PopulationStats {
            total_strategies: self.current_population.len(),
            average_age,
            strategy_type_distribution: strategy_type_dist,
            performance_distribution,
        }
    }

    fn calculate_performance_distribution(&self, scores: &[f64]) -> PerformanceDistribution {
        if scores.is_empty() {
            return PerformanceDistribution {
                mean_fitness: 0.0,
                median_fitness: 0.0,
                std_fitness: 0.0,
                percentile_95: 0.0,
                percentile_75: 0.0,
                percentile_25: 0.0,
                percentile_5: 0.0,
            };
        }

        let mut sorted_scores = scores.to_vec();
        sorted_scores.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mean = scores.iter().sum::<f64>() / scores.len() as f64;
        let variance = scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / scores.len() as f64;
        let std_dev = variance.sqrt();

        PerformanceDistribution {
            mean_fitness: mean,
            median_fitness: sorted_scores[sorted_scores.len() / 2],
            std_fitness: std_dev,
            percentile_95: sorted_scores[(sorted_scores.len() as f64 * 0.95) as usize],
            percentile_75: sorted_scores[(sorted_scores.len() as f64 * 0.75) as usize],
            percentile_25: sorted_scores[(sorted_scores.len() as f64 * 0.25) as usize],
            percentile_5: sorted_scores[(sorted_scores.len() as f64 * 0.05) as usize],
        }
    }

    fn calculate_evolution_metrics(&self) -> EvolutionMetrics {
        // Calculate convergence rate (how quickly population is converging)
        let fitness_variance = if self.evolution_history.len() >= 2 {
            let recent = &self.evolution_history[self.evolution_history.len() - 1];
            let previous = &self.evolution_history[self.evolution_history.len() - 2];
            (previous.fitness_variance - recent.fitness_variance) / previous.fitness_variance
        } else {
            0.0
        };

        // Diversity index (strategy type diversity)
        let mut type_counts = HashMap::new();
        for candidate in &self.current_population {
            *type_counts
                .entry(candidate.strategy_type.clone())
                .or_insert(0) += 1;
        }

        let diversity_index = if !type_counts.is_empty() {
            let total = self.current_population.len() as f64;
            -type_counts
                .values()
                .map(|&count| {
                    let p = count as f64 / total;
                    p * p.ln()
                })
                .sum::<f64>()
        } else {
            0.0
        };

        let (adaptation_speed, stability_score, innovation_rate) = if let Some(recent) =
            self.evolution_history.back()
        {
            let previous = self.evolution_history.iter().rev().nth(1);

            let adaptation_speed = previous
                .map(|prev| {
                    let denom = prev.average_fitness.abs().max(1e-6);
                    ((recent.average_fitness - prev.average_fitness) / denom).clamp(-1.0, 1.0)
                })
                .unwrap_or(0.0);

            let stability_score = {
                let fitness_spread = (recent.best_fitness - recent.worst_fitness).abs().max(1e-6);
                let variance = recent.fitness_variance.max(0.0);
                let std_dev = variance.sqrt();
                (1.0 - (std_dev / fitness_spread).min(1.0)).clamp(0.0, 1.0)
            };

            let innovation_rate = if recent.population_size > 0 {
                let created = recent.new_strategies_created as f64;
                let churn = (recent.new_strategies_created + recent.strategies_eliminated) as f64;
                if churn > 0.0 {
                    (created / churn).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            } else {
                0.0
            };

            (adaptation_speed, stability_score, innovation_rate)
        } else {
            (0.0, 0.0, 0.0)
        };

        EvolutionMetrics {
            convergence_rate: fitness_variance,
            diversity_index,
            adaptation_speed,
            stability_score,
            innovation_rate,
        }
    }

    fn generate_recommendations(&self, _metrics: &EvolutionMetrics) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Analyze population and suggest actions
        let avg_fitness = self.calculate_average_fitness();
        if avg_fitness < 0.3 {
            recommendations
                .push("Population fitness is low - consider increasing mutation rate".to_string());
        }

        if self.current_population.len() < self.population_size / 2 {
            recommendations.push(
                "Population size is below optimal - consider adding new seed strategies"
                    .to_string(),
            );
        }

        let elite_ratio = self.elite_size as f64 / self.current_population.len() as f64;
        if elite_ratio > 0.5 {
            recommendations.push("Elite ratio is high - increase population diversity".to_string());
        }

        recommendations
    }

    fn get_current_generation(&self) -> u32 {
        self.strategy_generations
            .values()
            .max()
            .copied()
            .unwrap_or(0)
    }

    fn calculate_average_fitness(&self) -> f64 {
        if self.current_population.is_empty() {
            return 0.0;
        }

        self.current_population
            .iter()
            .map(|c| c.fitness_score)
            .sum::<f64>()
            / self.current_population.len() as f64
    }
}

impl Default for CandidateMetrics {
    fn default() -> Self {
        Self {
            sharpe_ratio: 0.0,
            total_return: 0.0,
            max_drawdown: 0.0,
            win_rate: 0.0,
            trade_count: 0,
            volatility: 0.0,
            calmar_ratio: 0.0,
            sortino_ratio: 0.0,
            stability_score: 0.5,
        }
    }
}
