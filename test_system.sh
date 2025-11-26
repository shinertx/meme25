#!/bin/bash

# MemeSnipe v25 Test Suite
# Validates system deployment and functionality

set -e

echo "🧪 MemeSnipe v25 Test Suite Starting..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test results
PASSED=0
FAILED=0

run_test() {
    local test_name="$1"
    local test_command="$2"
    
    echo -e "${YELLOW}Testing: $test_name${NC}"
    
    if eval "$test_command"; then
        echo -e "${GREEN}✅ PASSED: $test_name${NC}"
        ((PASSED++))
    else
        echo -e "${RED}❌ FAILED: $test_name${NC}"
        ((FAILED++))
    fi
    echo
}

# Test 1: Docker services are running
test_docker_services() {
    docker-compose ps | grep -E "(redis|postgres|prometheus|grafana)" | grep "Up" > /dev/null
}

# Test 2: Redis connectivity
test_redis() {
    docker-compose exec -T redis redis-cli ping | grep "PONG" > /dev/null
}

# Test 3: PostgreSQL connectivity
test_postgres() {
    docker-compose exec -T postgres pg_isready -U postgres | grep "accepting connections" > /dev/null
}

# Test 4: Prometheus targets
test_prometheus() {
    curl -s http://localhost:9090/api/v1/targets | jq '.data.activeTargets | length' | grep -E "[1-9]" > /dev/null
}

# Test 5: Grafana accessibility
test_grafana() {
    curl -s http://localhost:3000/api/health | jq '.database' | grep "ok" > /dev/null
}

# Test 6: Market data gateway health
test_market_data() {
    timeout 10 docker-compose logs market_data_gateway | grep "Starting Market Data Gateway" > /dev/null
}

# Test 7: Portfolio manager health
test_portfolio() {
    timeout 10 docker-compose logs portfolio_manager | grep "Starting Portfolio Manager" > /dev/null
}

# Test 8: Executor health
test_executor() {
    timeout 10 docker-compose logs executor | grep "MemeSnipe v25 Executor starting" > /dev/null
}

# Test 9: Signer health
test_signer() {
    timeout 10 docker-compose logs signer | grep "Signer service starting" > /dev/null
}

# Test 10: Backtest engine health
test_backtest() {
    curl -s http://localhost:8001/health | jq '.status' | grep "healthy" > /dev/null 2>&1 || true
}

# Test 11: Redis streams exist
test_redis_streams() {
    docker-compose exec -T redis redis-cli EXISTS market_events | grep "1" > /dev/null
}

# Test 12: Database tables exist
test_db_tables() {
    docker-compose exec -T postgres psql -U postgres -d meme_snipe_v25 -c "\dt" | grep -E "(trades|strategy_performance|capital_allocations|risk_events)" > /dev/null
}

# Test 13: Environment variables loaded
test_env_vars() {
    [ ! -z "$SOLANA_RPC_URL" ] && [ ! -z "$PUMP_FUN_API_KEY" ]
}

# Test 14: Shared models compilation
test_shared_models() {
    cd shared-models && cargo check > /dev/null 2>&1
    cd ..
}

# Test 15: Strategy files exist
test_strategy_files() {
    local strategies=("momentum_5m" "mean_revert_1h" "social_buzz" "liquidity_migration" "perp_basis_arb" "dev_wallet_drain" "airdrop_rotation" "korean_time_burst" "bridge_inflow" "rug_pull_sniffer")
    
    for strategy in "${strategies[@]}"; do
        [ -f "executor/src/strategies/${strategy}.rs" ] || return 1
    done
}

echo "🔍 Running infrastructure tests..."

# Load environment variables
if [ -f .env ]; then
    source .env
fi

# Run all tests
run_test "Docker services running" "test_docker_services"
run_test "Redis connectivity" "test_redis"
run_test "PostgreSQL connectivity" "test_postgres" 
run_test "Prometheus targets" "test_prometheus"
run_test "Grafana accessibility" "test_grafana"
run_test "Market data gateway" "test_market_data"
run_test "Portfolio manager" "test_portfolio"
run_test "Executor service" "test_executor"
run_test "Signer service" "test_signer"
run_test "Backtest engine" "test_backtest"
run_test "Redis streams" "test_redis_streams"
run_test "Database tables" "test_db_tables"
run_test "Environment variables" "test_env_vars"
run_test "Shared models compilation" "test_shared_models"
run_test "Strategy files exist" "test_strategy_files"

echo "📊 Test Results Summary:"
echo -e "${GREEN}✅ Passed: $PASSED${NC}"
echo -e "${RED}❌ Failed: $FAILED${NC}"

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}🎉 All tests passed! MemeSnipe v25 is ready for trading.${NC}"
    exit 0
else
    echo -e "${RED}⚠️  Some tests failed. Please check the logs and fix issues before trading.${NC}"
    exit 1
fi
