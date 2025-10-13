#!/bin/bash

# MemeSnipe v25 Autonomous Trading CLI
# Trigger autonomous strategy evolution and deployment

set -e

echo "🤖 MemeSnipe v25 Autonomous Trading System"
echo "=========================================="

# Check if paper trading mode
PAPER_MODE=${PAPER_TRADING_MODE:-true}
if [ "$PAPER_MODE" = "true" ]; then
    echo "📊 Running in Paper Trading Mode (Full Autonomy Enabled)"
else
    echo "💰 Running in Live Trading Mode (Human Approval Required)"
fi

case "${1:-help}" in
    "evolve")
        echo "🧬 Starting Autonomous Strategy Evolution..."
        echo "   - Generating new strategies via genetic algorithm"
        echo "   - Auto-testing with historical data"
        echo "   - Auto-committing profitable strategies to GitHub"
        echo "   - Auto-deploying to paper trading"
        echo ""
        
        # Start evolution engine
        cd /home/benjaminjones/meme25
        RUST_LOG=info cargo run --bin evolution_engine
        ;;
        
    "backtest")
        echo "📈 Running Autonomous Backtesting..."
        echo "   - Testing all strategies against recent market data"
        echo "   - Generating performance reports"
        echo "   - Auto-promoting best performers"
        echo ""
        
        cd /home/benjaminjones/meme25
        python backtest_engine/run_backtest.py
        ;;
        
    "deploy")
        echo "🚀 Deploying Trading System..."
        echo "   - Starting all services"
        echo "   - Validating data pipelines"
        echo "   - Beginning autonomous operation"
        echo ""
        
        cd /home/benjaminjones/meme25
        docker-compose -f docker-compose.efficient.yml up -d
        echo "✅ System deployed and running autonomously"
        ;;
        
    "status")
        echo "📊 System Status:"
        cd /home/benjaminjones/meme25
        
        # Check if services are running
        if docker-compose -f docker-compose.efficient.yml ps | grep -q "Up"; then
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
        
        cd /home/benjaminjones/meme25
        
        # Stream logs from key services
        docker-compose -f docker-compose.efficient.yml logs -f executor strategy_factory | \
        grep -E "(auto-generated|evolution|strategy|profit|loss)" --color=always
        ;;
        
    "stop")
        echo "🛑 Stopping Autonomous Trading System..."
        cd /home/benjaminjones/meme25
        docker-compose -f docker-compose.efficient.yml down
        echo "✅ System stopped"
        ;;
        
    "help"|*)
        echo ""
        echo "Available Commands:"
        echo "  evolve   - Start autonomous strategy evolution (full AI loop)"
        echo "  backtest - Run backtesting on existing strategies"
        echo "  deploy   - Deploy trading system for autonomous operation"
        echo "  status   - Check system status and recent AI activity"
        echo "  monitor  - Stream live autonomous trading activity"
        echo "  stop     - Stop all autonomous trading"
        echo ""
        echo "Examples:"
        echo "  ./autonomous_cli.sh evolve     # Start AI strategy generation"
        echo "  ./autonomous_cli.sh deploy     # Deploy for autonomous trading"
        echo "  ./autonomous_cli.sh monitor    # Watch AI in action"
        echo ""
        echo "The system operates with full autonomy in paper trading mode:"
        echo "  • AI generates new strategies"
        echo "  • Auto-tests with backtesting"
        echo "  • Auto-commits profitable code to GitHub"
        echo "  • Auto-deploys to paper trading"
        echo "  • Continuously evolves and improves"
        ;;
esac