#!/bin/bash
# Bootstrap script for initializing Vault and secrets
# Critical Finding #13: Secure secret handling

set -e

echo "🔐 Initializing MemeSnipe v25 Vault and Secrets"

# Default values
VAULT_ADDR=${VAULT_ADDR:-"http://localhost:8200"}
VAULT_TOKEN=${VAULT_TOKEN:-"memesnipe-dev-token"}

# Wait for Vault to be ready
echo "⏳ Waiting for Vault to be ready..."
until curl -s "${VAULT_ADDR}/v1/sys/health" >/dev/null 2>&1; do
    echo "Waiting for Vault at ${VAULT_ADDR}..."
    sleep 2
done

echo "✅ Vault is ready"

# Export token for subsequent commands
export VAULT_ADDR
export VAULT_TOKEN

# Enable KV v2 secrets engine
echo "🔧 Enabling KV v2 secrets engine..."
vault secrets enable -path=secret kv-v2 2>/dev/null || echo "KV engine already enabled"

# Generate Solana keypair if it doesn't exist
KEYPAIR_PATH="./secrets/solana-keypair.json"
if [ ! -f "$KEYPAIR_PATH" ]; then
    echo "🔑 Generating new Solana keypair..."
    mkdir -p ./secrets
    # In production, this would use a proper key generation method
    echo '[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63,64]' > "$KEYPAIR_PATH"
    echo "⚠️  WARNING: Using dummy keypair for development. Replace with real keypair in production!"
fi

# Store keypair in Vault
echo "💾 Storing Solana keypair in Vault..."
vault kv put secret/solana keypair=@"$KEYPAIR_PATH"

# Store API keys in Vault (using environment variables if available)
echo "🔐 Storing API keys in Vault..."
vault kv put secret/api-keys \
    helius="${HELIUS_API_KEY:-dummy-helius-key}" \
    jupiter="${JUPITER_API_KEY:-dummy-jupiter-key}" \
    pump_fun="${PUMP_FUN_API_KEY:-dummy-pump-fun-key}" \
    birdeye="${BIRDEYE_API_KEY:-dummy-birdeye-key}" \
    twitter="${TWITTER_BEARER_TOKEN:-dummy-twitter-token}" \
    farcaster="${FARCASTER_API_KEY:-dummy-farcaster-key}"

# Store database credentials
echo "💽 Storing database credentials in Vault..."
vault kv put secret/database \
    url="${DATABASE_URL:-postgres://postgres:postgres@localhost:5432/meme_snipe_v25}" \
    username="postgres" \
    password="${DB_PASSWORD:-postgres}"

# Store Redis URL
echo "📊 Storing Redis configuration in Vault..."
vault kv put secret/redis \
    url="${REDIS_URL:-redis://localhost:6379}"

echo "✅ Vault initialization complete!"
echo ""
echo "📋 Next steps:"
echo "1. Update your .env file with VAULT_ADDR and VAULT_TOKEN"
echo "2. Replace dummy API keys with real ones in production"
echo "3. Generate a real Solana keypair for production use"
echo ""
echo "🔧 To access secrets:"
echo "   vault kv get secret/solana"
echo "   vault kv get secret/api-keys"
echo "   vault kv get secret/database"
