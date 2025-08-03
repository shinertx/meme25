-- MemeSnipe v25 Database Schema
-- Critical Finding #10: Database Schema Implementation

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pg_stat_statements";

-- Trades table for tracking all executed trades
CREATE TABLE IF NOT EXISTS trades (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    strategy_id VARCHAR(100) NOT NULL,
    symbol VARCHAR(50) NOT NULL,
    side VARCHAR(10) NOT NULL CHECK (side IN ('BUY', 'SELL')),
    amount_usd DECIMAL(18, 8) NOT NULL,
    price DECIMAL(18, 8) NOT NULL,
    confidence DECIMAL(3, 2) NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    executed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    tx_hash VARCHAR(88), -- Solana transaction hash
    status VARCHAR(20) DEFAULT 'PENDING' CHECK (status IN ('PENDING', 'EXECUTED', 'FAILED', 'CANCELLED')),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Portfolio positions tracking
CREATE TABLE IF NOT EXISTS positions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    symbol VARCHAR(50) NOT NULL,
    token_address VARCHAR(44) NOT NULL, -- Solana address
    amount DECIMAL(18, 8) NOT NULL,
    average_price DECIMAL(18, 8) NOT NULL,
    current_price DECIMAL(18, 8),
    unrealized_pnl DECIMAL(18, 8) DEFAULT 0,
    realized_pnl DECIMAL(18, 8) DEFAULT 0,
    opened_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(symbol, token_address)
);

-- Strategy performance metrics
CREATE TABLE IF NOT EXISTS strategy_performance (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    strategy_id VARCHAR(100) NOT NULL,
    total_trades INTEGER DEFAULT 0,
    winning_trades INTEGER DEFAULT 0,
    losing_trades INTEGER DEFAULT 0,
    total_pnl DECIMAL(18, 8) DEFAULT 0,
    max_drawdown DECIMAL(5, 4) DEFAULT 0,
    sharpe_ratio DECIMAL(8, 4),
    win_rate DECIMAL(5, 4),
    avg_win DECIMAL(18, 8),
    avg_loss DECIMAL(18, 8),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(strategy_id)
);

-- Market events log for debugging and analysis
CREATE TABLE IF NOT EXISTS market_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_type VARCHAR(50) NOT NULL,
    symbol VARCHAR(50),
    data JSONB NOT NULL,
    processed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Risk events and alerts
CREATE TABLE IF NOT EXISTS risk_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_type VARCHAR(50) NOT NULL,
    severity VARCHAR(20) NOT NULL CHECK (severity IN ('LOW', 'MEDIUM', 'HIGH', 'CRITICAL')),
    description TEXT NOT NULL,
    metadata JSONB,
    resolved BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    resolved_at TIMESTAMP WITH TIME ZONE
);

-- Portfolio snapshots for historical analysis
CREATE TABLE IF NOT EXISTS portfolio_snapshots (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    total_value_usd DECIMAL(18, 8) NOT NULL,
    available_cash_usd DECIMAL(18, 8) NOT NULL,
    total_positions INTEGER NOT NULL,
    daily_pnl DECIMAL(18, 8) NOT NULL,
    total_pnl DECIMAL(18, 8) NOT NULL,
    drawdown_pct DECIMAL(5, 4) NOT NULL,
    snapshot_time TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Indexes for performance (Fix #14: Enhanced database indexes)
CREATE INDEX IF NOT EXISTS idx_trades_strategy_id ON trades(strategy_id);
CREATE INDEX IF NOT EXISTS idx_trades_symbol ON trades(symbol);
CREATE INDEX IF NOT EXISTS idx_trades_executed_at ON trades(executed_at);
CREATE INDEX IF NOT EXISTS idx_trades_status ON trades(status);
CREATE INDEX IF NOT EXISTS idx_trades_strategy_symbol ON trades(strategy_id, symbol);
CREATE INDEX IF NOT EXISTS idx_trades_executed_at_desc ON trades(executed_at DESC);

CREATE INDEX IF NOT EXISTS idx_positions_symbol ON positions(symbol);
CREATE INDEX IF NOT EXISTS idx_positions_token_address ON positions(token_address);
CREATE INDEX IF NOT EXISTS idx_positions_updated_at ON positions(updated_at);

CREATE INDEX IF NOT EXISTS idx_market_events_type ON market_events(event_type);
CREATE INDEX IF NOT EXISTS idx_market_events_symbol ON market_events(symbol);
CREATE INDEX IF NOT EXISTS idx_market_events_created_at ON market_events(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_market_events_type_symbol ON market_events(event_type, symbol);

CREATE INDEX IF NOT EXISTS idx_risk_events_type ON risk_events(event_type);
CREATE INDEX IF NOT EXISTS idx_risk_events_severity ON risk_events(severity);
CREATE INDEX IF NOT EXISTS idx_risk_events_resolved ON risk_events(resolved);
CREATE INDEX IF NOT EXISTS idx_risk_events_created_at ON risk_events(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_portfolio_snapshots_time ON portfolio_snapshots(snapshot_time);
CREATE INDEX IF NOT EXISTS idx_portfolio_snapshots_time_desc ON portfolio_snapshots(snapshot_time DESC);

CREATE INDEX IF NOT EXISTS idx_strategy_performance_strategy_id ON strategy_performance(strategy_id);
CREATE INDEX IF NOT EXISTS idx_strategy_performance_updated_at ON strategy_performance(updated_at);

-- Functions for automatic timestamp updates
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Triggers for automatic timestamp updates
CREATE TRIGGER update_trades_updated_at BEFORE UPDATE ON trades
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_positions_updated_at BEFORE UPDATE ON positions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_strategy_performance_updated_at BEFORE UPDATE ON strategy_performance
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
