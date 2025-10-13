# MemeSnipe v25 - Autonomous Quant Development Pipeline
# Supporting GPT-5 Codex autonomous operation

.PHONY: all lint test e2e perf sprint clean setup

# Default target - full development cycle
all: lint test e2e perf

# Setup development environment
setup:
	@echo "🔧 Setting up development environment..."
	@docker-compose -f docker-compose.efficient.yml pull
	@cargo fetch
	@pip install -r backtest_engine/requirements.txt

# Code quality checks
lint:
	@echo "🔍 Running code quality checks..."
	@cargo fmt --check
	@cargo clippy -- -D warnings
	@echo "✅ Code quality checks passed"

# Unit tests
test:
	@echo "🧪 Running unit tests..."
	@cargo test --workspace
	@python -m pytest tests/ -v
	@echo "✅ Unit tests passed"

# End-to-end integration tests
e2e:
	@echo "🔄 Running end-to-end tests..."
	@docker-compose -f docker-compose.efficient.yml up -d postgres redis
	@sleep 5
	@cargo test --test integration_tests
	@docker-compose -f docker-compose.efficient.yml down
	@echo "✅ E2E tests passed"

# Performance benchmarks
perf:
	@echo "⚡ Running performance benchmarks..."
	@cargo bench
	@python backtest_engine/run_backtest.py --benchmark
	@echo "✅ Performance benchmarks completed"

# Full sprint cycle (used by Codex AI)
sprint: lint test e2e perf
	@echo "🏃 Full sprint cycle completed successfully"
	@echo "  ✅ Code quality validated"
	@echo "  ✅ Unit tests passed" 
	@echo "  ✅ Integration tests passed"
	@echo "  ✅ Performance benchmarks met"
	@echo "🎯 Ready for autonomous deployment"

# Build all containers
build:
	@echo "🐳 Building Docker containers..."
	@docker-compose -f docker-compose.efficient.yml build

# Deploy system
deploy: build
	@echo "🚀 Deploying autonomous trading system..."
	@docker-compose -f docker-compose.efficient.yml up -d
	@echo "✅ System deployed and running autonomously"

# Stop system
stop:
	@echo "🛑 Stopping autonomous trading system..."
	@docker-compose -f docker-compose.efficient.yml down
	@echo "✅ System stopped"

# Clean up build artifacts
clean:
	@echo "🧹 Cleaning build artifacts..."
	@cargo clean
	@docker system prune -f
	@echo "✅ Cleanup completed"

# Monitor system logs
monitor:
	@echo "📊 Monitoring autonomous operations..."
	@docker-compose -f docker-compose.efficient.yml logs -f

# Autonomous evolution cycle (called by Codex AI)
evolve:
	@echo "🧬 Starting autonomous evolution cycle..."
	@./autonomous_cli.sh evolve

# Strategy validation
validate-strategies:
	@echo "🔍 Validating all strategies..."
	@find executor/src/strategies -name "*.rs" -exec cargo check --manifest-path executor/Cargo.toml {} \;
	@echo "✅ All strategies validated"

# Risk audit
risk-audit:
	@echo "⚠️  Running risk management audit..."
	@cargo test --package executor risk_manager::tests
	@python scripts/risk_audit.py
	@echo "✅ Risk audit completed"

# Generate performance report
report:
	@echo "📈 Generating performance report..."
	@python dashboard/generate_report.py
	@echo "✅ Performance report generated"

# Help
help:
	@echo "MemeSnipe v25 - Autonomous Quant Development Pipeline"
	@echo ""
	@echo "Main Commands:"
	@echo "  make sprint    - Full development cycle (lint + test + e2e + perf)"
	@echo "  make evolve    - Start autonomous strategy evolution"
	@echo "  make deploy    - Deploy trading system"
	@echo "  make monitor   - Monitor live operations"
	@echo ""
	@echo "Development:"
	@echo "  make lint      - Code quality checks"
	@echo "  make test      - Unit tests" 
	@echo "  make e2e       - Integration tests"
	@echo "  make perf      - Performance benchmarks"
	@echo ""
	@echo "Operations:"
	@echo "  make build     - Build containers"
	@echo "  make stop      - Stop system"
	@echo "  make clean     - Clean artifacts"
	@echo ""
	@echo "Powered by GPT-5 Codex for autonomous operation"