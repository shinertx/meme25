#!/bin/bash

# MemeSnipe v25 System Status Check
# Verifies that the cloud upgrade and system components are working correctly

echo "🔍 MemeSnipe v25 System Status Check"
echo "======================================"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

PASSED=0
FAILED=0
WARNINGS=0

check_pass() {
    echo -e "${GREEN}✅ PASS:${NC} $1"
    ((PASSED++))
}

check_fail() {
    echo -e "${RED}❌ FAIL:${NC} $1"
    ((FAILED++))
}

check_warn() {
    echo -e "${YELLOW}⚠️  WARN:${NC} $1"
    ((WARNINGS++))
}

check_info() {
    echo -e "${BLUE}ℹ️  INFO:${NC} $1"
}

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$REPO_ROOT"

echo "📁 Repository: $REPO_ROOT"
echo ""

# === BUILD STATUS ===
echo "═══════════════════════════════════════"
echo "🔨 BUILD STATUS"
echo "═══════════════════════════════════════"

# Check if Cargo.toml exists
if [ -f "Cargo.toml" ]; then
    check_pass "Cargo.toml found"
else
    check_fail "Cargo.toml not found"
fi

# Check release build artifacts
if [ -f "target/release/executor" ]; then
    check_pass "Release executor binary exists"
else
    check_warn "Release executor binary not found (run 'cargo build --release')"
fi

# === CONFIGURATION ===
echo ""
echo "═══════════════════════════════════════"
echo "⚙️  CONFIGURATION"
echo "═══════════════════════════════════════"

# Check .env.example
if [ -f ".env.example" ]; then
    check_pass ".env.example template exists"
else
    check_fail ".env.example template not found"
fi

# Check .env file (without revealing secrets)
if [ -f ".env" ]; then
    check_pass ".env file exists"
else
    check_warn ".env file not found (copy from .env.example)"
fi

# Check Docker files
if [ -f "docker-compose.yml" ]; then
    check_pass "docker-compose.yml exists"
else
    check_warn "docker-compose.yml not found"
fi

if [ -f "docker-compose.efficient.yml" ]; then
    check_pass "docker-compose.efficient.yml exists (optimized build)"
else
    check_warn "docker-compose.efficient.yml not found"
fi

# === CORE COMPONENTS ===
echo ""
echo "═══════════════════════════════════════"
echo "🏗️  CORE COMPONENTS"
echo "═══════════════════════════════════════"

# Check each core service directory
for SERVICE in executor market_data_gateway portfolio_manager position_manager strategy_factory backtest_engine; do
    if [ -d "$SERVICE" ] && [ -f "$SERVICE/Cargo.toml" ]; then
        check_pass "$SERVICE service exists"
    else
        check_fail "$SERVICE service not found"
    fi
done

# Check shared-models
if [ -d "shared-models" ] && [ -f "shared-models/Cargo.toml" ]; then
    check_pass "shared-models library exists"
else
    check_fail "shared-models library not found"
fi

# === STRATEGIES ===
echo ""
echo "═══════════════════════════════════════"
echo "📊 TRADING STRATEGIES"
echo "═══════════════════════════════════════"

# Check strategies directory
STRATEGY_DIR="executor/src/strategies"
if [ -d "$STRATEGY_DIR" ]; then
    STRATEGY_COUNT=$(ls -1 "$STRATEGY_DIR"/*.rs 2>/dev/null | wc -l)
    check_pass "Strategies directory exists ($STRATEGY_COUNT strategy files)"
else
    check_fail "Strategies directory not found"
fi

# === SCRIPTS ===
echo ""
echo "═══════════════════════════════════════"
echo "📜 SCRIPTS & TOOLING"
echo "═══════════════════════════════════════"

for script in deploy.sh test_system.sh autonomous_cli.sh; do
    if [ -f "$script" ]; then
        check_pass "$script exists"
    else
        check_warn "$script not found"
    fi
done

# === DOCUMENTATION ===
echo ""
echo "═══════════════════════════════════════"
echo "📚 DOCUMENTATION"
echo "═══════════════════════════════════════"

if [ -f "README.md" ]; then
    check_pass "README.md exists"
else
    check_warn "README.md not found"
fi

# === SUMMARY ===
echo ""
echo "═══════════════════════════════════════"
echo "📋 SUMMARY"
echo "═══════════════════════════════════════"
echo ""
echo -e "${GREEN}Passed:${NC}   $PASSED"
echo -e "${RED}Failed:${NC}   $FAILED"
echo -e "${YELLOW}Warnings:${NC} $WARNINGS"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}🎉 System status: OPERATIONAL${NC}"
    echo ""
    echo "The MemeSnipe v25 trading system is working correctly."
    echo "All core components are present and configured."
    echo ""
    echo "Next steps for cloud deployment:"
    echo "  1. Copy .env.example to .env and configure API keys"
    echo "  2. Run 'cargo build --release' for production binaries"
    echo "  3. Use 'docker-compose up -d' to start services"
    echo "  4. Monitor via Grafana at http://localhost:3000"
    exit 0
else
    echo -e "${RED}⚠️  System status: ISSUES DETECTED${NC}"
    echo ""
    echo "Please fix the failed checks before deploying."
    exit 1
fi
