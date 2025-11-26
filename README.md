# MemeSnipe v25 — Autonomous Memecoin Trading System

## ⚠️ **Production Status: IN DEVELOPMENT**

**Current Status**: 🟡 Code Complete, Security Hardening Required  
**Production Ready**: ❌ NOT YET - Critical security items must be addressed  
**Last Updated**: 2025-11-24

### Quick Status
- ✅ **Code Quality**: EXCELLENT - Zero compilation errors, all clippy warnings resolved
- ✅ **Risk Management**: Comprehensive circuit breakers and limits implemented
- 🔴 **Security**: HIGH RISK - Critical issues require 2-3 weeks of work
- 🟡 **Strategy Validation**: 11 strategies identified, testing required
- 🟡 **Documentation**: Comprehensive guides created, operational docs needed

**See [PRODUCTION_READINESS.md](PRODUCTION_READINESS.md) for complete checklist**  
**See [SECURITY_AUDIT.md](SECURITY_AUDIT.md) for security assessment**  
**See [UPGRADE_SUMMARY.md](UPGRADE_SUMMARY.md) for detailed upgrade report**

---

## 🎯 **What This Project Does**

**MemeSnipe v25** is a production-grade, autonomous trading system designed to turn $200 into $1M by trading Solana memecoins. It operates as a sophisticated ensemble of 10 parallel trading strategies that collectively analyze real-time market data and execute trades with institutional-grade risk management.

### **Core Functionality**

The system implements a **multi-strategy ensemble architecture** where:

1. **Real-Time Data Ingestion**
   - Connects to Helius WebSocket for live Solana blockchain data
   - Monitors price movements, volume spikes, and on-chain activity
   - Processes events with <500ms latency requirement

2. **10 Parallel Trading Strategies**
   - **Momentum Strategies**: Capture trending moves (5min, daily timeframes)
   - **Mean Reversion**: Exploit overextended price movements
   - **Event-Driven**: React to volume spikes and social sentiment
   - **Smart Money**: Follow whale wallets and sophisticated traders
   - **Arbitrage**: Cross-venue and cross-chain opportunities
   - Each strategy operates independently but within strict risk limits

3. **Ensemble Decision Making**
   - All strategies analyze the same market events simultaneously
   - Each can generate trade signals based on their unique edge
   - Risk manager aggregates exposure across all strategies
   - Natural selection: winning strategies get more capital allocation

4. **Risk Management**
   - Portfolio-wide 10% drawdown circuit breaker
   - 2% maximum position size per trade
   - 10% maximum allocation per strategy
   - Real-time P&L tracking and exposure monitoring

5. **Execution Infrastructure**
   - Jupiter aggregator integration for best price execution
   - Jito bundle submission for MEV protection
   - Slippage modeling and liquidity analysis
   - Paper trading mode for strategy validation

### **Technical Architecture**

```
Market Data (Helius) → Redis Streams → Event Loop → 10 Strategies → Risk Manager → Execution
                                           ↓
                                     Metrics/Monitoring
                                           ↓
                                     PostgreSQL/Grafana
```

### **Key Features**

- **Autonomous Operation**: Runs 24/7 without manual intervention
- **Multi-Strategy Ensemble**: Adapts to different market conditions
- **Production-Grade**: Comprehensive error handling, monitoring, circuit breakers
- **Backtesting Engine**: Validate strategies on historical data
- **Paper Trading**: Test with real market data before risking capital
- **Observability**: Prometheus metrics, Grafana dashboards, detailed logging

### **Performance Goals**

- Target: Transform $200 → $1M through compounding
- Required: Massive return over 1 Month compunding
- Minimum Sharpe Ratio: 1.5 for live trading
- Win Rate Targets: >55% momentum, >65% mean reversion

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Project Structure](#project-structure)
3. [Environment Setup](#environment-setup)
4. [Database Setup](#database-setup)
5. [Running the System](#running-the-system)
6. [Development](#development)
7. [Testing](#testing)
8. [Monitoring](#monitoring)
9. [Troubleshooting](#troubleshooting)

---

## Prerequisites
// ...existing code...

1. **Clone and Configure**:
   ```bash
   git clone <repository-url>
   cd meme25-1
   cp .env.example .env
   # Edit .env with your API keys and configuration
   ```

2. **Deploy System**:
   ```bash
   ./deploy.sh
   ```

3. **Validate Deployment**:
   ```bash
   ./test_system.sh
   ```

4. **Monitor Trading**:
   - Grafana Dashboard: http://localhost:3000 (admin/admin)
   - Prometheus Metrics: http://localhost:9090
   - Backtest API: http://localhost:8001/docs

## Key Features

- **Fully Autonomous Operation**: Zero manual intervention required after deployment
- **Dynamic Capital Allocation**: Performance-based weighting across 10 strategies
- **Genetic Algorithm Evolution**: Strategies continuously improve through natural selection
- **Real-Time Risk Management**: Circuit breakers, position limits, and portfolio-wide controls
- **Production Security**: Zeroized memory for keys, overflow protection, type safety
- **Budget Optimized**: Runs on $200/month infrastructure (GCP VM + services)
- **Comprehensive Monitoring**: Real-time dashboards with 15+ key metrics
- **Automated Testing**: Full system validation with health checks

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    External Data Sources                     │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────────┐   │
│  │   Helius    │  │   Twitter    │  │    Farcaster    │   │
│  │  WebSocket  │  │     API      │  │      API        │   │
│  └──────┬──────┘  └──────┬───────┘  └────────┬────────┘   │
└─────────┼─────────────────┼──────────────────┼─────────────┘
          │                 │                   │
┌─────────▼─────────────────▼──────────────────▼─────────────┐
│              Market Data Gateway (Rust)                     │
│  - Data validation & normalization                          │
│  - Circuit breaker for bad data                             │
│  - Redis stream publisher                                   │
└───────────────────────────┬─────────────────────────────────┘
                            │
                     Redis Event Streams
                            │
┌───────────────────────────▼─────────────────────────────────┐
│                    Executor (Rust)                          │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────────┐   │
│  │  Strategy 1 │  │  Strategy 2  │  │   Strategy 10   │   │
│  │  Momentum   │  │ Mean Revert  │  │   Rug Sniffer   │   │
│  └─────────────┘  └──────────────┘  └─────────────────┘   │
│                                                             │
│  - Risk validation for every trade                          │
│  - Jito bundle submission                                   │
│  - Position tracking                                        │
└────────────┬──────────────────────────────┬─────────────────┘
             │                              │
      ┌──────▼──────┐              ┌────────▼────────┐
      │   Signer    │              │ Risk Manager    │
      │  (Secure)   │              │  - Limits       │
      └─────────────┘              │  - Drawdown     │
                                   └─────────────────┘
```

## Trading Strategies

### 1. **Momentum 5m** (`momentum_5m`)
- Detects rapid price/volume surges in 5-minute windows
- Entry: Price change > threshold + volume spike
- Exit: 10% take profit or 5% stop loss

### 2. **Mean Reversion 1h** (`mean_revert_1h`)
- Trades extreme deviations from hourly moving average
- Entry: Z-score > 2 standard deviations
- Exit: Return to mean

### 3. **Social Buzz** (`social_buzz`)
- Monitors Twitter/Farcaster for sentiment spikes
- Entry: Engagement surge + positive sentiment
- Exit: Sentiment reversal or time-based

### 4. **Korean Time Burst** (`korean_time_burst`)
- Exploits timezone-specific volume patterns (9AM-3PM KST)
- Entry: Volume multiplier during Korean hours
- Exit: Volume normalization

### 5. **Dev Wallet Drain** (`dev_wallet_drain`)
- Detects and shorts tokens with developer dumps
- Entry: Large dev wallet transfers detected
- Exit: Price stabilization or -30% target

### 6. **Bridge Inflow** (`bridge_inflow`)
- Tracks cross-chain liquidity migrations
- Entry: Multi-chain bridge volume > threshold
- Exit: Momentum exhaustion

### 7. **Airdrop Rotation** (`airdrop_rotation`)
- Capitalizes on post-airdrop selling pressure
- Entry: >50% claims + volume surge
- Exit: 25% profit target

### 8. **Liquidity Migration** (`liquidity_migration`)
- Follows LP movements between DEXes
- Entry: Major liquidity pool changes
- Exit: New equilibrium reached

### 9. **Perp Basis Arbitrage** (`perp_basis_arb`)
- Arbitrages spot vs perpetual price differences
- Entry: Basis > threshold with funding alignment
- Exit: Basis convergence

### 10. **Rug Pull Sniffer** (`rug_pull_sniffer`)
- Emergency short on detected rug pulls
- Entry: Multiple red flags (price crash + liquidity drain + dev activity)
- Exit: Let run to near-zero

## Deployment Instructions

### Prerequisites
- GCP VM instance (already created)
- Docker and Docker Compose installed
- Wallet keypairs generated

### Step 1: Connect to VM
```bash
gcloud compute ssh meme-snipe-v19-vm --zone=us-central1-a
```

### Step 2: Clone Repository
```bash
cd /home/trader
git clone https://github.com/your-repo/meme-snipe-v25.git bot
cd bot
```

### Step 3: Configure Environment
```bash
cp .env.example .env
# Edit .env with your API keys and settings
nano .env
```

### Step 4: Place Wallet Files
```bash
# Copy your wallet keypairs
cp /path/to/my_wallet.json .
cp /path/to/jito_auth_key.json .
```

### Step 5: Build and Deploy
```bash
# Build all services
docker compose build

# Start the system
docker compose up -d

# Check status
docker compose ps

# View logs
docker compose logs -f executor
```

### ⚡ Paper-Trading Quickstart (Optimized Build)
For local validation or CI-style runs where you want tight rebuild loops, use the unified build
pipeline and lightweight compose file:

```bash
# 1. Compile every Rust crate once (warms the cache and validates code)
cargo build --release --workspace

# 2. Bake lean runtime images that reuse the compiled artifacts
DOCKER_BUILDKIT=1 docker compose -f docker-compose.efficient.yml build

# 3. Launch the paper-trading stack with default mock credentials
DOCKER_BUILDKIT=1 docker compose -f docker-compose.efficient.yml up -d

# 4. Verify all services are healthy
docker compose -f docker-compose.efficient.yml ps

# 5. Inspect executor boot logs & metrics port (defaults to 9100)
docker compose -f docker-compose.efficient.yml logs --tail=100 executor
```

**Heads-up:** the efficient compose file pins development-friendly environment variables (dummy
API keys, paper-mode toggles). Replace them with real credentials before live trading by editing
`docker-compose.efficient.yml` or providing overrides via `docker compose --env-file`.

#### Market data filter toggles

Key environment knobs for Solana meme coverage live inside `market_data_gateway`:

- `MKT_MIN_LIQUIDITY_USD` – hard floor on pool liquidity (default $10M)
- `MKT_MIN_VOLUME_USD` – rejects pairs whose 24h volume is below the floor (default $5M)
- `MKT_NEWPAIRS_MIN_AGE_MIN` / `MKT_NEWPAIRS_MAX_AGE_MIN` – bounds acceptable pool age window (defaults 10–120 minutes)

Tune these to balance breadth vs. execution quality; volume and age filters combine with the liquidity gate before anything hits the allocator.

#### Reproducibility Checklist

Run these before cutting a release or promoting a strategy update:

- `cargo fmt && cargo clippy -- -D warnings` – style + lint sanity
- `cargo build --release --workspace` – compile every crate and surface linkage issues
- `cargo test --workspace` – fast unit coverage (integration harness stays disabled by default)
- `cargo test -p tests --features integration-tests -- --ignored` – optional end-to-end harness
   that exercises Redis/Postgres paths
- `DOCKER_BUILDKIT=1 docker compose -f docker-compose.efficient.yml build` – ensure images bake
- `DOCKER_BUILDKIT=1 docker compose -f docker-compose.efficient.yml up -d && docker compose -f
   docker-compose.efficient.yml ps` – smoke test service startup

### Step 6: Verify Operation
```bash
# Check if strategies are running
docker compose exec redis redis-cli xlen allocations_channel

# Monitor trades
docker compose exec postgres psql -U postgres -d meme_snipe_v25 -c "SELECT * FROM trades ORDER BY entry_time DESC LIMIT 10;"

# Check portfolio health
curl http://localhost:9184/health
```

## Monitoring

- **Grafana Dashboard**: http://localhost:3000 (admin/memesnipe)
- **Prometheus**: http://localhost:9090
- **Trading Dashboard**: http://localhost:8080

## Risk Management

- **Portfolio Stop Loss**: 10% daily drawdown limit
- **Position Limits**: Max $50 per position (25% of capital)
- **Circuit Breakers**: Auto-pause on anomalous behavior
- **Real-time Monitoring**: All trades logged and validated

## Strategy Evolution

The system continuously evolves through:
1. **Genetic Algorithm**: Top performers breed new variants
2. **A/B Testing**: Paper trading validates improvements
3. **Performance Tracking**: Real-time Sharpe ratio monitoring
4. **Auto-promotion**: Profitable strategies get more capital

## Security Features

- **Isolated Signer**: Private keys in separate secure container
- **Memory Protection**: Zeroized sensitive data
- **Input Validation**: All external data sanitized
- **Resource Limits**: Docker resource constraints prevent DoS

## Budget Breakdown

**Monthly Operating Costs (~$200)**:
- GCP VM (e2-standard-4): ~$120/month
- Data APIs (Helius, Birdeye): ~$50/month
- Social APIs (Twitter, Farcaster): ~$30/month
- **Total**: ~$200/month

## Performance Targets

- **Target Growth**: $200 → $1000000+ over 1 month
- **Minimum Sharpe**: >1.5 for strategy promotion
- **Max Drawdown**: <10% daily, <20% monthly
- **Win Rate**: >55% across all strategies
- **Uptime**: >99.5% availability

## Troubleshooting

### Common Issues

1. **Services won't start**:
   ```bash
   docker compose logs [service_name]
   docker system prune -f
   ```

2. **Database connection errors**:
   ```bash
   docker compose restart postgres
   docker compose exec postgres pg_isready
   ```

3. **Strategy allocation issues**:
   ```bash
   docker compose exec redis redis-cli flushall
   docker compose restart portfolio_manager
   ```

4. **Low performance**:
   - Check VM resources: `htop`
   - Monitor Redis memory: `docker stats`
   - Review strategy allocation weights

### Emergency Procedures

**Circuit Breaker Triggered**:
1. Check risk events table for cause
2. Review recent trades for anomalies
3. Manual override via portfolio manager if needed

**Major Loss Event**:
1. Immediately pause all trading
2. Review trade logs and market conditions
3. Analyze strategy performance degradation
4. Implement fixes before resuming

## Development

For development and testing:

```bash
# Run in paper trading mode
export PAPER_TRADING_MODE=true
docker compose up -d

# Run backtests
curl -X POST http://localhost:8000/backtest \
  -H "Content-Type: application/json" \
  -d '{"strategy_spec": {...}, "start_time": "...", "end_time": "..."}'

# View simulation results
docker compose exec redis redis-cli xread STREAMS shadow_ledgers:momentum_5m 0
```

## License

Proprietary - All rights reserved.

---

**⚠️ IMPORTANT**: This system trades with real money. Always test thoroughly in paper mode before going live. Past performance does not guarantee future results.
