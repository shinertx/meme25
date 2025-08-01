CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Strategies table
CREATE TABLE strategies (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    family VARCHAR(100) NOT NULL,
    parameters JSONB NOT NULL DEFAULT '{}',
    fitness_score DECIMAL(10,4) DEFAULT 0.0,
    status VARCHAR(50) DEFAULT 'active',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Trades table
CREATE TABLE trades (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    strategy_id UUID REFERENCES strategies(id),
    token_address VARCHAR(44) NOT NULL,
    side VARCHAR(4) CHECK (side IN ('BUY', 'SELL')),
    amount_usd DECIMAL(18,8) NOT NULL,
    price_usd DECIMAL(18,8) NOT NULL,
    status VARCHAR(20) DEFAULT 'PENDING',
    signature VARCHAR(88),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    executed_at TIMESTAMP WITH TIME ZONE,
    pnl_usd DECIMAL(18,8) DEFAULT 0.0
);

-- Market data table
CREATE TABLE market_data (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    token_address VARCHAR(44) NOT NULL,
    price_usd DECIMAL(18,8) NOT NULL,
    volume_24h DECIMAL(18,2),
    market_cap DECIMAL(18,2),
    liquidity_usd DECIMAL(18,2),
    timestamp TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Social signals table
CREATE TABLE social_signals (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    token_address VARCHAR(44) NOT NULL,
    platform VARCHAR(50) NOT NULL,
    signal_type VARCHAR(50) NOT NULL,
    sentiment_score DECIMAL(5,3),
    volume_mentions INTEGER DEFAULT 0,
    metadata JSONB DEFAULT '{}',
    timestamp TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Portfolio snapshots
CREATE TABLE portfolio_snapshots (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    total_value_usd DECIMAL(18,8) NOT NULL,
    pnl_24h DECIMAL(18,8) DEFAULT 0.0,
    active_positions INTEGER DEFAULT 0,
    snapshot_data JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX idx_trades_strategy_id ON trades(strategy_id);
CREATE INDEX idx_trades_token_address ON trades(token_address);
CREATE INDEX idx_trades_created_at ON trades(created_at);
CREATE INDEX idx_market_data_token_address ON market_data(token_address);
CREATE INDEX idx_market_data_timestamp ON market_data(timestamp);
CREATE INDEX idx_social_signals_token_address ON social_signals(token_address);
CREATE INDEX idx_social_signals_timestamp ON social_signals(timestamp);
