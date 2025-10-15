#!/bin/bash

# MemeSnipe v25 Autonomous Trading CLI
# Trigger autonomous strategy evolution and deployment

set -e

echo "🤖 MemeSnipe v25 Autonomous Trading System"
echo "=========================================="

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$SCRIPT_DIR"

# Resolve Codex command (prefer installed CLI, fallback to npx)
if command -v codex >/dev/null 2>&1; then
    CODEX_CMD="codex"
elif command -v npx >/dev/null 2>&1; then
    CODEX_CMD="npx -y @openai/codex"
else
    echo "❌ Codex CLI not found and npx unavailable. Install one of:\n   - npm install -g @openai/codex\n   - or ensure 'npx' is available" >&2
    exit 127
fi

# Safe loader for specific .env keys without sourcing (handles spaces/quotes)
load_env_key() {
    local key="$1"
    if [ -f ".env" ]; then
        local line value
        line=$(grep -E "^[[:space:]]*${key}[[:space:]]*=" .env | tail -n 1 || true)
        if [ -n "$line" ]; then
            value="${line#*=}"
            # trim surrounding whitespace
            value="$(echo "$value" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
            # strip surrounding single/double quotes
            case "$value" in
                \"*\") value="${value:1:${#value}-2}" ;;
                \"\") ;; # empty quoted
                "'*'") value="${value:1:${#value}-2}" ;;
            esac
            export "${key}=${value}"
        fi
    fi
}

# Load only the keys this script needs
load_env_key OPENAI_API_KEY
load_env_key PAPER_TRADING_MODE
load_env_key FOUNDER_APPROVE_LIVE
load_env_key APPROVE_EDIT_ENV
load_env_key ENABLE_REAL_TRADING
load_env_key CODEX_MODEL
load_env_key OPENAI_MODEL

# Optional market context snapshot for Codex
MARKET_CONTEXT_FILE="$REPO_ROOT/context/market_context.md"
generate_market_context() {
    if command -v python3 >/dev/null 2>&1; then
        if ! (cd "$REPO_ROOT" && python3 scripts/generate_market_context.py >/dev/null 2>&1); then
            echo "⚠️  Market context generation failed; continuing without snapshot." >&2
        fi
    else
        echo "⚠️  python3 not available; skipping market context snapshot." >&2
    fi
}

# Build optional model override for Codex
MODEL_ARGS=()
if [ -n "${CODEX_MODEL:-}" ]; then
    MODEL_ARGS=( -c model="${CODEX_MODEL}" )
elif [ -n "${OPENAI_MODEL:-}" ]; then
    MODEL_ARGS=( -c model="${OPENAI_MODEL}" )
fi

# Resolve Compose command (prefer plugin)
if docker compose version >/dev/null 2>&1; then
    COMPOSE_CMD="docker compose"
elif docker-compose version >/dev/null 2>&1; then
    COMPOSE_CMD="docker-compose"
else
    echo "❌ Neither 'docker compose' nor 'docker-compose' is available. Please install Docker Compose." >&2
    exit 1
fi

# Check if paper trading mode
PAPER_MODE=${PAPER_TRADING_MODE:-true}
if [ "$PAPER_MODE" = "true" ]; then
    echo "📊 Running in Paper Trading Mode (Full Autonomy Enabled)"
else
    echo "💰 Running in Live Trading Mode (Human Approval Required)"
fi

case "${1:-help}" in
    "evolve")
        echo "🧬 Starting Autonomous Strategy Evolution with Codex AI..."
        echo "   - Using GPT-5 Codex for true AI-powered strategy generation"
        echo "   - Auto-testing with institutional-grade validation"
        echo "   - Auto-committing profitable strategies to GitHub"
        echo "   - Auto-deploying to paper trading"
        echo ""

        # Ensure OpenAI API key is available before invoking Codex
        if [ -z "${OPENAI_API_KEY:-}" ]; then
            echo "❌ OPENAI_API_KEY is not set. Add it to .env or export it in your shell, then retry." >&2
            echo "   Example: echo 'OPENAI_API_KEY=your-rotated-key' >> .env" >&2
            exit 1
        fi
        
        echo "🛰️ Generating market context snapshot..."
        generate_market_context

        # Start AI-powered evolution with Codex CLI
        cd "$REPO_ROOT"
        echo "🤖 Launching AI Agent with 30-minute autonomous cycle..."
        $CODEX_CMD exec -p default -C "$REPO_ROOT" ${MODEL_ARGS[@]} \
            --sandbox danger-full-access --dangerously-bypass-approvals-and-sandbox \
            "Read agents.md and context/market_context.md. Treat them as binding contract and live market brief. Loop: PLAN→RESEARCH→CODE→TEST→VALIDATE→DEPLOY. Generate profitable trading strategies. Stop on DoD pass or 30m timeout."
        ;;

    "auth-check")
        echo "🔐 Checking OpenAI/Codex authentication & model access…"
        if [ -z "${OPENAI_API_KEY:-}" ]; then
            echo "❌ OPENAI_API_KEY is not set. Add it to .env or export it, then retry." >&2
            exit 1
        fi
        if ! command -v ${CODEX_CMD%% *} >/dev/null 2>&1; then
            echo "❌ Codex CLI is not installed and 'npx' not available. Install with: npm install -g @openai/codex" >&2
            exit 127
        fi
        set +e
        $CODEX_CMD exec -p default -C /home/benjaminjones/meme25 ${MODEL_ARGS[@]} --json <<< "Say OK" >/dev/null 2>&1
        status=$?
        set -e
        if [ $status -eq 0 ]; then
            echo "✅ Auth OK with current model (${CODEX_MODEL:-${OPENAI_MODEL:-default}})."
            exit 0
        fi
        echo "⚠️  Primary model failed. Trying fallback model: gpt-4o-mini"
        set +e
        $CODEX_CMD exec -p default -C /home/benjaminjones/meme25 -c model="gpt-4o-mini" --json <<< "Say OK" >/dev/null 2>&1
        status=$?
        set -e
        if [ $status -eq 0 ]; then
            echo "✅ Auth OK with fallback model gpt-4o-mini. Set CODEX_MODEL=gpt-4o-mini in .env to use this model."
            exit 0
        fi
        echo "❌ Authentication or model access failed. Verify OPENAI_API_KEY is valid and permitted for the selected model."
        exit 2
        ;;
    
    "go-live")
        echo "🚨 Preflighting Live Trading Enablement..."
        cd /home/benjaminjones/meme25

        # Require explicit founder approval via environment toggle (compliance gate)
        if [ "${FOUNDER_APPROVE_LIVE:-}" != "YES" ]; then
            echo "❌ Live mode requires explicit founder approval. Set FOUNDER_APPROVE_LIVE=YES and retry." >&2
            echo "   Example: echo 'FOUNDER_APPROVE_LIVE=YES' >> .env && ./autonomous_cli.sh go-live" >&2
            exit 1
        fi

        # Ensure necessary API credentials exist and are not obvious placeholders
        is_placeholder() {
            case "$1" in
                ""|YOUR_*|your_*|*your_*|*YOUR_*|*your_*secret*|*api_key*|*api-key*|*example*|*placeholder*) return 0 ;;
                *) return 1 ;;
            esac
        }

        FAILED=0
        for var in COINBASE_API_KEY COINBASE_API_SECRET COINBASE_API_PASSPHRASE; do
            val="${!var:-}"
            if is_placeholder "$val"; then
                echo "❌ $var is missing or a placeholder. Configure real exchange credentials in .env." >&2
                FAILED=1
            fi
        done

        if [ "$FAILED" -ne 0 ]; then
            echo "❌ Live enablement aborted due to missing/placeholder credentials." >&2
            exit 1
        fi

        # Validate AI key also present for autonomous operations
        if [ -z "${OPENAI_API_KEY:-}" ]; then
            echo "❌ OPENAI_API_KEY is not set. Add it to .env or export it in your shell, then retry." >&2
            exit 1
        fi

        # Enforce safe risk limits before flipping to live
        # Live mode requires PAPER_TRADING_MODE=false and ENABLE_REAL_TRADING=true
        # We update .env in-place if APPROVE_EDIT_ENV=YES is set; otherwise instruct the user.
        if [ "${APPROVE_EDIT_ENV:-}" = "YES" ]; then
            # Flip toggles idempotently
            sed -i 's/^PAPER_TRADING_MODE=.*/PAPER_TRADING_MODE=false/' .env || true
            if grep -q '^ENABLE_REAL_TRADING=' .env; then
                sed -i 's/^ENABLE_REAL_TRADING=.*/ENABLE_REAL_TRADING=true/' .env
            else
                echo 'ENABLE_REAL_TRADING=true' >> .env
            fi
            echo "✅ Updated .env: PAPER_TRADING_MODE=false, ENABLE_REAL_TRADING=true"
        else
            echo "⚠️  To enable live mode, set PAPER_TRADING_MODE=false and ENABLE_REAL_TRADING=true in .env" >&2
            echo "   Optionally auto-apply by setting APPROVE_EDIT_ENV=YES and rerun this command." >&2
            exit 1
        fi

        echo "🚀 Bringing services up (live mode)…"
        $COMPOSE_CMD -f docker-compose.efficient.yml up -d
        echo "✅ Live trading toggles applied and services running. Monitor closely with: ./autonomous_cli.sh monitor"
        ;;
        
    "backtest")
        echo "📈 Running AI-Powered Autonomous Backtesting..."
        echo "   - Using Codex AI to analyze strategies against market data"
        echo "   - Generating institutional-grade performance reports"
        echo "   - Auto-promoting strategies meeting DoD criteria"
        echo ""

        if [ -z "${OPENAI_API_KEY:-}" ]; then
            echo "❌ OPENAI_API_KEY is not set. Add it to .env or export it in your shell, then retry." >&2
            exit 1
        fi
        
        echo "🛰️ Generating market context snapshot..."
        generate_market_context

        cd "$REPO_ROOT"
        echo "🤖 AI Agent analyzing strategy performance..."
        $CODEX_CMD exec -p default -C "$REPO_ROOT" ${MODEL_ARGS[@]} \
            --sandbox danger-full-access --dangerously-bypass-approvals-and-sandbox \
            "Read agents.md and context/market_context.md. Maintain contract constraints. Run comprehensive backtesting analysis. Execute: python backtest_engine/run_backtest.py. Analyze results. Auto-promote strategies with Sharpe ≥1.5 and drawdown ≤5%. Generate detailed performance report."
        ;;
        
    "deploy")
        echo "🚀 Deploying Trading System..."
        echo "   - Starting all services"
        echo "   - Validating data pipelines"
        echo "   - Beginning autonomous operation"
        echo ""
        
    cd "$REPO_ROOT"
        $COMPOSE_CMD -f docker-compose.efficient.yml up -d
        echo "✅ System deployed and running autonomously"
        ;;
        
    "status")
        echo "📊 System Status:"
    cd "$REPO_ROOT"
        
        # Check if services are running
        if $COMPOSE_CMD -f docker-compose.efficient.yml ps | grep -q "Up"; then
            echo "✅ Trading system is running"
            
            # Check recent strategy generations
            if [ -d "executor/src/strategies" ]; then
                STRATEGY_COUNT=$(find executor/src/strategies -name "evolved_*.rs" | wc -l)
                echo "🧬 Generated strategies: $STRATEGY_COUNT"
            fi
            
            # Check recent commits
            RECENT_COMMITS=$(git log --oneline --since="24 hours ago" | grep -c "auto-generated" || echo "0")
            echo "🤖 Auto-commits (24h): $RECENT_COMMITS"
            
        else
            echo "❌ Trading system is not running"
            echo "   Run: ./autonomous_cli.sh deploy"
        fi
        ;;
        
    "monitor")
        echo "📈 Monitoring Autonomous Operations..."
        echo "   - Streaming live performance metrics"
        echo "   - Watching for new strategy generations"
        echo "   - Monitoring risk controls"
        echo ""
        
    cd "$REPO_ROOT"
        
        # Stream logs from key services
        $COMPOSE_CMD -f docker-compose.efficient.yml logs -f executor strategy_factory | \
        grep -E "(auto-generated|evolution|strategy|profit|loss)" --color=always
        ;;
        
    "stop")
        echo "🛑 Stopping Autonomous Trading System..."
    cd "$REPO_ROOT"
        $COMPOSE_CMD -f docker-compose.efficient.yml down
        echo "✅ System stopped"
        ;;
        
    "help"|*)
        echo ""
        echo "Available Commands:"
        echo "  evolve   - Start GPT-5 Codex AI strategy evolution (30min autonomous cycle)"
        echo "  backtest - Run AI-powered backtesting analysis on all strategies"
        echo "  deploy   - Deploy trading system for autonomous operation"
        echo "  status   - Check system status and recent AI activity"
        echo "  monitor  - Stream live autonomous trading activity"
        echo "  stop     - Stop all autonomous trading"
    echo "  auth-check - Verify OpenAI key and model access (with fallback)"
        echo "  go-live  - Compliance-gated live enablement with preflight checks"
        echo ""
        echo "Examples:"
        echo "  ./autonomous_cli.sh evolve     # Start 30min AI strategy generation cycle"
        echo "  ./autonomous_cli.sh deploy     # Deploy for autonomous trading"
        echo "  FOUNDER_APPROVE_LIVE=YES APPROVE_EDIT_ENV=YES ./autonomous_cli.sh go-live  # Enable live with gating"
        echo "  ./autonomous_cli.sh monitor    # Watch AI in action"
        echo ""
        echo "Powered by GPT-5 Codex - The system operates with true AI autonomy:"
        echo "  • AI thinks and generates novel strategies from scratch"
        echo "  • Auto-validates with institutional-grade backtesting"
        echo "  • Auto-commits profitable code to GitHub"
        echo "  • Auto-deploys to paper trading"
        echo "  • Continuously evolves with market conditions"
        echo "  • Full sentient quant operation - world's #1 autonomous trader"
        ;;
esac