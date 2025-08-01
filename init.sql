-- MemeSnipe v25 Database Schema
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Trades table
CREATE TABLE IF NOT EXISTS trades (
    id BIGSERIAL PRIMARY KEY,
    trade_uuid UUID DEFAULT uuid_generate_v4() UNIQUE NOT NULL,
    strategy_id TEXT NOT NULL,
    token_address TEXT NOT NULL,
    symbol TEXT NOT NULL,
    side VARCHAR(10) NOT NULL CHECK (side IN ('Long', 'Short')),
    amount_usd DECIMAL(20,8) NOT NULL,
    amount_tokens DECIMAL(40,18),
    status VARCHAR(20) NOT NULL,
    signature TEXT,
    entry_time TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    entry_price_usd DECIMAL(20,8) NOT NULL,
    entry_sol_price DECIMAL(20,8),
    close_time TIMESTAMP WITH TIME ZONE,
    close_price_usd DECIMAL(20,8),
    close_sol_price DECIMAL(20,8),
    pnl_usd DECIMAL(20,8),
    pnl_percent DECIMAL(10,4),
    fees_usd DECIMAL(20,8) DEFAULT 0,
    confidence DECIMAL(5,4) NOT NULL,
    highest_price_usd DECIMAL(20,8),
    lowest_price_usd DECIMAL(20,8),
    slippage_bps INTEGER,
    execution_time_ms INTEGER,
    mode VARCHAR(20) DEFAULT 'Live' CHECK (mode IN ('Simulating', 'Paper', 'Live')),
    metadata JSONB
);

-- Indexes for performance
CREATE INDEX idx_trades_strategy_id ON trades(strategy_id);
CREATE INDEX idx_trades_status ON trades(status);
CREATE INDEX idx_trades_entry_time ON trades(entry_time);
CREATE INDEX idx_trades_token_address ON trades(token_address);
CREATE INDEX idx_trades_mode ON trades(mode);

-- Strategy performance tracking
CREATE TABLE IF NOT EXISTS strategy_performance (
    id BIGSERIAL PRIMARY KEY,
    strategy_id TEXT NOT NULL,
    timestamp TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    total_trades INTEGER DEFAULT 0,
    winning_trades INTEGER DEFAULT 0,
    losing_trades INTEGER DEFAULT 0,
    total_pnl_usd DECIMAL(20,8) DEFAULT 0,
    sharpe_ratio DECIMAL(10,4),
    sortino_ratio DECIMAL(10,4),
    max_drawdown_percent DECIMAL(10,4),
    win_rate DECIMAL(5,4),
    avg_win_usd DECIMAL(20,8),
    avg_loss_usd DECIMAL(20,8),
    profit_factor DECIMAL(10,4),
    mode VARCHAR(20) DEFAULT 'Live' CHECK (mode IN ('Simulating', 'Paper', 'Live'))
);

CREATE INDEX idx_strategy_performance_strategy_id ON strategy_performance(strategy_id);
CREATE INDEX idx_strategy_performance_timestamp ON strategy_performance(timestamp);

-- Capital allocations
CREATE TABLE IF NOT EXISTS capital_allocations (
    id BIGSERIAL PRIMARY KEY,
    strategy_id TEXT NOT NULL,
    allocation_time TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    capital_allocated_usd DECIMAL(20,8) NOT NULL,
    weight DECIMAL(5,4) NOT NULL,
    mode VARCHAR(20) NOT NULL CHECK (mode IN ('Simulating', 'Paper', 'Live')),
    reason TEXT
);

CREATE INDEX idx_capital_allocations_strategy_id ON capital_allocations(strategy_id);
CREATE INDEX idx_capital_allocations_time ON capital_allocations(allocation_time);

-- Risk events and circuit breakers
CREATE TABLE IF NOT EXISTS risk_events (
    id BIGSERIAL PRIMARY KEY,
    event_time TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    event_type VARCHAR(50) NOT NULL,
    severity VARCHAR(20) CHECK (severity IN ('LOW', 'MEDIUM', 'HIGH', 'CRITICAL')),
    strategy_id TEXT,
    description TEXT NOT NULL,
    action_taken TEXT,
    metadata JSONB
);

CREATE INDEX idx_risk_events_time ON risk_events(event_time);
CREATE INDEX idx_risk_events_type ON risk_events(event_type);

-- Market data cache
CREATE TABLE IF NOT EXISTS market_data_cache (
    id BIGSERIAL PRIMARY KEY,
    token_address TEXT NOT NULL,
    timestamp TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    price_usd DECIMAL(20,8) NOT NULL,
    volume_24h_usd DECIMAL(20,8),
    liquidity_usd DECIMAL(20,8),
    market_cap_usd DECIMAL(20,8),
    price_change_24h_percent DECIMAL(10,4),
    holders_count INTEGER,
    source VARCHAR(50),
    metadata JSONB
);

CREATE INDEX idx_market_data_cache_token ON market_data_cache(token_address);
CREATE INDEX idx_market_data_cache_time ON market_data_cache(timestamp);

-- Historical backtests
CREATE TABLE IF NOT EXISTS backtests (
    id BIGSERIAL PRIMARY KEY,
    backtest_id UUID DEFAULT uuid_generate_v4() UNIQUE NOT NULL,
    strategy_id TEXT NOT NULL,
    start_time TIMESTAMP WITH TIME ZONE NOT NULL,
    end_time TIMESTAMP WITH TIME ZONE NOT NULL,
    initial_capital DECIMAL(20,8) NOT NULL,
    final_capital DECIMAL(20,8),
    total_return_percent DECIMAL(10,4),
    sharpe_ratio DECIMAL(10,4),
    max_drawdown_percent DECIMAL(10,4),
    total_trades INTEGER,
    win_rate DECIMAL(5,4),
    status VARCHAR(20) CHECK (status IN ('PENDING', 'RUNNING', 'COMPLETED', 'FAILED')),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMP WITH TIME ZONE,
    error_message TEXT,
    results JSONB
);

CREATE INDEX idx_backtests_strategy_id ON backtests(strategy_id);
CREATE INDEX idx_backtests_status ON backtests(status);
