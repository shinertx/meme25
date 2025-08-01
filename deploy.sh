#!/bin/bash

# MemeSnipe v25 Deployment Script
set -e

echo "🚀 MemeSnipe v25 - Production Deployment Starting..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if running as root
if [[ $EUID -eq 0 ]]; then
   echo -e "${RED}This script should not be run as root${NC}"
   exit 1
fi

# Check if .env file exists
if [ ! -f ".env" ]; then
    echo -e "${YELLOW}Creating .env from template...${NC}"
    cp .env.example .env
    echo -e "${RED}Please edit .env file with your API keys and settings before continuing${NC}"
    echo "Required changes:"
    echo "  - Add your Helius API key"
    echo "  - Add your Farcaster API key"  
    echo "  - Add your Twitter Bearer token"
    echo "  - Add your Birdeye API key"
    echo "  - Set secure database password"
    echo "  - Place wallet keypair files (my_wallet.json, jito_auth_key.json)"
    exit 1
fi

# Check for wallet files
if [ ! -f "my_wallet.json" ]; then
    echo -e "${RED}Wallet file my_wallet.json not found${NC}"
    echo "Please place your Solana wallet keypair file as my_wallet.json"
    exit 1
fi

if [ ! -f "jito_auth_key.json" ]; then
    echo -e "${RED}Jito auth keypair jito_auth_key.json not found${NC}"
    echo "Please place your Jito auth keypair file as jito_auth_key.json"
    exit 1
fi

# Check Docker
if ! command -v docker &> /dev/null; then
    echo -e "${RED}Docker is not installed${NC}"
    exit 1
fi

if ! command -v docker-compose &> /dev/null; then
    echo -e "${RED}Docker Compose is not installed${NC}"
    exit 1
fi

# Create shared directory
mkdir -p shared

# Set up git if not already done
if [ ! -d ".git" ]; then
    echo -e "${YELLOW}Initializing git repository...${NC}"
    git init
    git add .
    git commit -m "Initial commit - MemeSnipe v25"
fi

echo -e "${GREEN}✅ Prerequisites check passed${NC}"

# Build and start services
echo -e "${YELLOW}Building Docker images...${NC}"
docker-compose build --no-cache

echo -e "${YELLOW}Starting services...${NC}"
docker-compose up -d

# Wait for services to be healthy
echo -e "${YELLOW}Waiting for services to be ready...${NC}"
sleep 30

# Check service health
echo -e "${YELLOW}Checking service health...${NC}"

# Check Redis
if docker-compose exec redis redis-cli ping | grep -q "PONG"; then
    echo -e "${GREEN}✅ Redis is healthy${NC}"
else
    echo -e "${RED}❌ Redis is not responding${NC}"
fi

# Check PostgreSQL
if docker-compose exec postgres pg_isready -U postgres | grep -q "accepting connections"; then
    echo -e "${GREEN}✅ PostgreSQL is healthy${NC}"
else
    echo -e "${RED}❌ PostgreSQL is not responding${NC}"
fi

# Check Signer
if curl -s http://localhost:8989/health | grep -q "OK"; then
    echo -e "${GREEN}✅ Signer service is healthy${NC}"
else
    echo -e "${RED}❌ Signer service is not responding${NC}"
fi

# Check Executor
if curl -s http://localhost:9184/health | grep -q "OK"; then
    echo -e "${GREEN}✅ Executor service is healthy${NC}"
else
    echo -e "${RED}❌ Executor service is not responding${NC}"
fi

echo ""
echo -e "${GREEN}🎉 MemeSnipe v25 Deployment Complete!${NC}"
echo ""
echo "Service URLs:"
echo "  🔍 Trading Dashboard: http://localhost:8080"
echo "  📊 Grafana: http://localhost:3000 (admin/memesnipe)"
echo "  📈 Prometheus: http://localhost:9090"
echo "  ⚡ Backtest Engine: http://localhost:8000"
echo ""
echo "Monitoring Commands:"
echo "  📋 Service Status: docker-compose ps"
echo "  📝 Service Logs: docker-compose logs -f [service_name]"
echo "  💾 Database Access: docker-compose exec postgres psql -U postgres -d meme_snipe_v25"
echo "  🔄 Redis Access: docker-compose exec redis redis-cli"
echo ""
echo "Next Steps:"
echo "  1. Monitor initial strategy allocation"
echo "  2. Check first trades in paper mode"
echo "  3. Verify all data sources are working"
echo "  4. Set up alerts and monitoring"
echo ""
echo -e "${YELLOW}⚠️  Remember: System starts in PAPER TRADING mode by default${NC}"
echo -e "${YELLOW}⚠️  Change PAPER_TRADING_MODE=false in .env when ready for live trading${NC}"
echo ""
echo "Happy trading! 🚀"
