#!/usr/bin/env python3
"""
MemeSnipe v25 Infrastructure Smoke Test

This script validates basic connectivity to all infrastructure components:
- Redis: Write/read a test key
- PostgreSQL: Execute a simple query
- Helius RPC: Ping the Solana RPC endpoint
- DexScreener: Verify external market data API
- Jupiter: Verify swap quote API

Run this script OUTSIDE Docker containers to test connectivity to running services.

Usage:
    ./scripts/smoke_test.py [--host-mode]

    --host-mode: Use localhost URLs instead of Docker service names
"""

import os
import sys
import json
import time
import argparse
from datetime import datetime
from typing import Tuple, Optional, Dict, Any

# Results tracking
results: Dict[str, Dict[str, Any]] = {}


def log_result(test_name: str, success: bool, message: str, latency_ms: Optional[float] = None):
    """Log test result with timestamp."""
    status = "✅ PASS" if success else "❌ FAIL"
    latency_str = f" ({latency_ms:.1f}ms)" if latency_ms else ""
    print(f"{status}: {test_name}{latency_str} - {message}")
    results[test_name] = {
        "success": success,
        "message": message,
        "latency_ms": latency_ms,
        "timestamp": datetime.utcnow().isoformat()
    }


def test_redis(redis_url: str) -> Tuple[bool, str]:
    """Test Redis connectivity by writing and reading a test key."""
    try:
        import redis as redis_lib
    except ImportError:
        return False, "redis-py not installed. Run: pip install redis"
    
    start = time.time()
    try:
        client = redis_lib.from_url(redis_url, socket_timeout=5)
        
        # Write test key
        test_key = "smoke_test:ping"
        test_value = f"pong_{int(time.time())}"
        client.set(test_key, test_value, ex=60)  # 60s expiry
        
        # Read test key
        read_value = client.get(test_key)
        if read_value is None:
            return False, "Failed to read back test key"
        
        read_value = read_value.decode() if isinstance(read_value, bytes) else read_value
        if read_value != test_value:
            return False, f"Value mismatch: expected {test_value}, got {read_value}"
        
        # Check stream exists (events:price)
        stream_info = client.xlen("events:price")
        
        latency = (time.time() - start) * 1000
        log_result("Redis Write/Read", True, f"Key written/read successfully. events:price has {stream_info} entries", latency)
        return True, f"Connected, stream has {stream_info} entries"
        
    except redis_lib.ConnectionError as e:
        latency = (time.time() - start) * 1000
        log_result("Redis Write/Read", False, f"Connection failed: {e}", latency)
        return False, str(e)
    except Exception as e:
        latency = (time.time() - start) * 1000
        log_result("Redis Write/Read", False, f"Unexpected error: {e}", latency)
        return False, str(e)


def test_postgres(database_url: str) -> Tuple[bool, str]:
    """Test PostgreSQL connectivity by executing a simple query."""
    try:
        import psycopg2
    except ImportError:
        return False, "psycopg2 not installed. Run: pip install psycopg2-binary"
    
    start = time.time()
    try:
        conn = psycopg2.connect(database_url, connect_timeout=5)
        cursor = conn.cursor()
        
        # Simple connectivity test
        cursor.execute("SELECT 1 AS test")
        result = cursor.fetchone()
        
        if result is None or result[0] != 1:
            return False, "Query returned unexpected result"
        
        # Check if trades table exists
        cursor.execute("""
            SELECT COUNT(*) FROM information_schema.tables 
            WHERE table_name = 'trades'
        """)
        table_exists = cursor.fetchone()[0] > 0
        
        # Count trades if table exists
        trade_count = 0
        if table_exists:
            cursor.execute("SELECT COUNT(*) FROM trades")
            trade_count = cursor.fetchone()[0]
        
        cursor.close()
        conn.close()
        
        latency = (time.time() - start) * 1000
        status_msg = f"Connected. trades table {'exists' if table_exists else 'missing'}, {trade_count} trades"
        log_result("PostgreSQL Query", True, status_msg, latency)
        return True, status_msg
        
    except psycopg2.OperationalError as e:
        latency = (time.time() - start) * 1000
        log_result("PostgreSQL Query", False, f"Connection failed: {e}", latency)
        return False, str(e)
    except Exception as e:
        latency = (time.time() - start) * 1000
        log_result("PostgreSQL Query", False, f"Unexpected error: {e}", latency)
        return False, str(e)


def test_helius_rpc(rpc_url: str) -> Tuple[bool, str]:
    """Test Helius/Solana RPC connectivity with getHealth."""
    try:
        import requests
    except ImportError:
        return False, "requests not installed. Run: pip install requests"
    
    start = time.time()
    try:
        payload = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getHealth"
        }
        
        response = requests.post(rpc_url, json=payload, timeout=10)
        latency = (time.time() - start) * 1000
        
        if response.status_code != 200:
            log_result("Helius RPC Health", False, f"HTTP {response.status_code}", latency)
            return False, f"HTTP {response.status_code}"
        
        data = response.json()
        if "error" in data:
            log_result("Helius RPC Health", False, f"RPC error: {data['error']}", latency)
            return False, f"RPC error: {data['error']}"
        
        result = data.get("result", "unknown")
        log_result("Helius RPC Health", True, f"Health: {result}", latency)
        return True, f"Health: {result}"
        
    except requests.RequestException as e:
        latency = (time.time() - start) * 1000
        log_result("Helius RPC Health", False, f"Request failed: {e}", latency)
        return False, str(e)
    except Exception as e:
        latency = (time.time() - start) * 1000
        log_result("Helius RPC Health", False, f"Unexpected error: {e}", latency)
        return False, str(e)


def test_dexscreener() -> Tuple[bool, str]:
    """Test DexScreener API connectivity."""
    try:
        import requests
    except ImportError:
        return False, "requests not installed. Run: pip install requests"
    
    start = time.time()
    try:
        url = "https://api.dexscreener.com/latest/dex/tokens/So11111111111111111111111111111111111111112"
        response = requests.get(url, timeout=10)
        latency = (time.time() - start) * 1000
        
        if response.status_code != 200:
            log_result("DexScreener API", False, f"HTTP {response.status_code}", latency)
            return False, f"HTTP {response.status_code}"
        
        data = response.json()
        pairs = data.get("pairs", [])
        pair_count = len(pairs)
        
        log_result("DexScreener API", True, f"Found {pair_count} pairs for SOL", latency)
        return True, f"Found {pair_count} pairs"
        
    except requests.RequestException as e:
        latency = (time.time() - start) * 1000
        log_result("DexScreener API", False, f"Request failed: {e}", latency)
        return False, str(e)
    except Exception as e:
        latency = (time.time() - start) * 1000
        log_result("DexScreener API", False, f"Unexpected error: {e}", latency)
        return False, str(e)


def test_jupiter() -> Tuple[bool, str]:
    """Test Jupiter quote API connectivity."""
    try:
        import requests
    except ImportError:
        return False, "requests not installed. Run: pip install requests"
    
    start = time.time()
    try:
        url = "https://quote-api.jup.ag/v6/quote"
        params = {
            "inputMint": "So11111111111111111111111111111111111111112",  # SOL
            "outputMint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",  # USDC
            "amount": "1000000",  # 0.001 SOL in lamports
            "slippageBps": "50"
        }
        
        response = requests.get(url, params=params, timeout=10)
        latency = (time.time() - start) * 1000
        
        if response.status_code != 200:
            log_result("Jupiter Quote API", False, f"HTTP {response.status_code}", latency)
            return False, f"HTTP {response.status_code}"
        
        data = response.json()
        out_amount = data.get("outAmount", "0")
        
        log_result("Jupiter Quote API", True, f"Quote received: {out_amount} USDC", latency)
        return True, f"Quote: {out_amount} USDC"
        
    except requests.RequestException as e:
        latency = (time.time() - start) * 1000
        log_result("Jupiter Quote API", False, f"Request failed: {e}", latency)
        return False, str(e)
    except Exception as e:
        latency = (time.time() - start) * 1000
        log_result("Jupiter Quote API", False, f"Unexpected error: {e}", latency)
        return False, str(e)


def test_redis_streams(redis_url: str) -> Tuple[bool, str]:
    """Test Redis streams for event pipeline data flow."""
    try:
        import redis as redis_lib
    except ImportError:
        return False, "redis-py not installed. Run: pip install redis"
    
    start = time.time()
    try:
        client = redis_lib.from_url(redis_url, socket_timeout=5)
        
        streams = [
            "events:price",
            "events:volume", 
            "events:social",
            "events:whale",
            "events:liquidation"
        ]
        
        stream_stats = {}
        for stream in streams:
            try:
                length = client.xlen(stream)
                stream_stats[stream] = length
            except Exception:
                stream_stats[stream] = -1  # -1 means stream doesn't exist
        
        active_streams = sum(1 for v in stream_stats.values() if v > 0)
        total_events = sum(v for v in stream_stats.values() if v > 0)
        
        latency = (time.time() - start) * 1000
        
        status_msg = f"{active_streams}/{len(streams)} active streams, {total_events} total events"
        log_result("Redis Streams", active_streams > 0, status_msg, latency)
        
        # Print stream details
        for stream, count in stream_stats.items():
            status = "📊" if count > 0 else "⚪"
            print(f"    {status} {stream}: {count if count >= 0 else 'not created'} entries")
        
        return active_streams > 0, status_msg
        
    except Exception as e:
        latency = (time.time() - start) * 1000
        log_result("Redis Streams", False, f"Error: {e}", latency)
        return False, str(e)


def check_env_vars() -> Tuple[bool, str]:
    """Check for required environment variables."""
    required = [
        "REDIS_URL",
        "DATABASE_URL",
        "SOLANA_RPC_URL",
        "HELIUS_API_KEY",
    ]
    
    optional_important = [
        "PAPER_TRADING_MODE",
        "JUPITER_BASE_URL",
        "BIRDEYE_API_KEY",
    ]
    
    missing_required = []
    missing_optional = []
    placeholder_vars = []
    
    for var in required:
        val = os.environ.get(var, "")
        if not val:
            missing_required.append(var)
        elif "YOUR_" in val or "REPLACE" in val or "PLACEHOLDER" in val:
            placeholder_vars.append(var)
    
    for var in optional_important:
        val = os.environ.get(var, "")
        if not val:
            missing_optional.append(var)
    
    success = len(missing_required) == 0 and len(placeholder_vars) == 0
    
    if missing_required:
        log_result("Environment Variables", False, f"Missing required: {', '.join(missing_required)}")
    elif placeholder_vars:
        log_result("Environment Variables", False, f"Placeholder values in: {', '.join(placeholder_vars)}")
    else:
        log_result("Environment Variables", True, f"All required vars set. Optional missing: {len(missing_optional)}")
    
    if missing_optional:
        print(f"    ⚠️  Optional vars not set: {', '.join(missing_optional)}")
    
    return success, f"Required: {len(required) - len(missing_required)}/{len(required)}"


def main():
    parser = argparse.ArgumentParser(description="MemeSnipe v25 Infrastructure Smoke Test")
    parser.add_argument("--host-mode", action="store_true", 
                       help="Use localhost URLs instead of Docker service names")
    parser.add_argument("--env-file", type=str, default=".env",
                       help="Path to .env file to load")
    args = parser.parse_args()
    
    # Load environment from file if it exists
    if os.path.exists(args.env_file):
        print(f"📄 Loading environment from {args.env_file}")
        with open(args.env_file) as f:
            for line in f:
                line = line.strip()
                if line and not line.startswith("#") and "=" in line:
                    key, _, value = line.partition("=")
                    # Don't override existing env vars
                    if key not in os.environ:
                        os.environ[key] = value
    
    print("\n" + "=" * 60)
    print("🔬 MemeSnipe v25 Infrastructure Smoke Test")
    print("=" * 60)
    print(f"Timestamp: {datetime.utcnow().isoformat()}Z")
    print(f"Mode: {'Host (localhost)' if args.host_mode else 'Docker (service names)'}")
    print("=" * 60 + "\n")
    
    # Determine URLs based on mode
    if args.host_mode:
        redis_url = os.environ.get("REDIS_URL", "redis://localhost:6379").replace("redis://redis:", "redis://localhost:")
        database_url = os.environ.get("DATABASE_URL_HOST", os.environ.get("DATABASE_URL", "")).replace("@postgres:", "@localhost:")
    else:
        redis_url = os.environ.get("REDIS_URL", "redis://redis:6379")
        database_url = os.environ.get("DATABASE_URL", "")
    
    helius_rpc = os.environ.get("HELIUS_RPC_URL", "") or os.environ.get("SOLANA_RPC_URL", "https://api.mainnet-beta.solana.com")
    
    print("📋 Configuration Check")
    print("-" * 40)
    check_env_vars()
    
    print("\n📡 Infrastructure Connectivity Tests")
    print("-" * 40)
    
    # Run tests
    all_passed = True
    
    # Redis tests
    success, _ = test_redis(redis_url)
    all_passed = all_passed and success
    
    success, _ = test_redis_streams(redis_url)
    # Streams being empty is not a failure, just informational
    
    # PostgreSQL test
    if database_url:
        success, _ = test_postgres(database_url)
        all_passed = all_passed and success
    else:
        log_result("PostgreSQL Query", False, "DATABASE_URL not set")
        all_passed = False
    
    print("\n🌐 External API Connectivity Tests")
    print("-" * 40)
    
    # Helius/Solana RPC test
    success, _ = test_helius_rpc(helius_rpc)
    all_passed = all_passed and success
    
    # DexScreener test
    success, _ = test_dexscreener()
    # DexScreener being down shouldn't fail the whole test
    
    # Jupiter test
    success, _ = test_jupiter()
    all_passed = all_passed and success
    
    # Summary
    print("\n" + "=" * 60)
    passed_count = sum(1 for r in results.values() if r["success"])
    total_count = len(results)
    
    if all_passed:
        print(f"✅ SMOKE TEST PASSED ({passed_count}/{total_count} tests)")
        print("   Infrastructure is ready for trading operations.")
    else:
        print(f"❌ SMOKE TEST FAILED ({passed_count}/{total_count} tests)")
        print("   Please fix the failing components before running the bot.")
        
        # List failures
        failures = [name for name, r in results.items() if not r["success"]]
        print(f"\n   Failed tests: {', '.join(failures)}")
    
    print("=" * 60 + "\n")
    
    # Write results to JSON for programmatic access
    output_file = "/tmp/smoke_test_results.json"
    with open(output_file, "w") as f:
        json.dump({
            "timestamp": datetime.utcnow().isoformat(),
            "all_passed": all_passed,
            "results": results
        }, f, indent=2)
    print(f"📝 Results written to {output_file}")
    
    sys.exit(0 if all_passed else 1)


if __name__ == "__main__":
    main()
