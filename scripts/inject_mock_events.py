#!/usr/bin/env python3
"""
MemeSnipe v25 Mock Event Injector

This script injects mock market events into Redis streams for testing
the executor and strategy pipeline without requiring live market data.

Use Cases:
1. Test strategy signal generation with "perfect trade" scenarios
2. Verify data flow from Redis to Executor
3. Debug strategy conditions that aren't triggering
4. Validate end-to-end paper trading flow

Usage:
    ./scripts/inject_mock_events.py --scenario momentum_spike
    ./scripts/inject_mock_events.py --scenario volume_surge
    ./scripts/inject_mock_events.py --custom --price 0.5 --volume 1000000 --liquidity 500000
"""

import os
import sys
import json
import time
import argparse
from datetime import datetime, timezone
from typing import Dict, Any, Optional

def get_redis_client(redis_url: str):
    """Create Redis client."""
    try:
        import redis as redis_lib
        return redis_lib.from_url(redis_url, socket_timeout=5)
    except ImportError:
        print("❌ redis-py not installed. Run: pip install redis")
        sys.exit(1)


def create_price_event(
    token_address: str,
    price_usd: float,
    volume_5m: float,
    liquidity_usd: float,
    price_change_5m: float = 0.0,
) -> Dict[str, Any]:
    """Create a PriceTick event wrapped in Event::Market format."""
    timestamp = datetime.now(timezone.utc).isoformat()
    
    # This matches the shared_models::Event::Market(MarketEvent::Price(PriceTick)) structure
    return {
        "Market": {
            "Price": {
                "token_address": token_address,
                "price_usd": price_usd,
                "volume_usd_1m": volume_5m / 5,
                "volume_usd_5m": volume_5m,
                "volume_usd_15m": volume_5m * 3,
                "price_change_1m": price_change_5m / 5,
                "price_change_5m": price_change_5m,
                "liquidity_usd": liquidity_usd,
                "timestamp": timestamp
            }
        }
    }


def create_volume_event(
    token_address: str,
    volume_spike_ratio: float,
    buy_volume: float,
    sell_volume: float,
) -> Dict[str, Any]:
    """Create a VolumeEvent wrapped in Event::Market format."""
    timestamp = datetime.now(timezone.utc).isoformat()
    
    return {
        "Market": {
            "Volume": {
                "token_address": token_address,
                "volume_spike_ratio": volume_spike_ratio,
                "buy_volume_usd": buy_volume,
                "sell_volume_usd": sell_volume,
                "unique_traders": int(buy_volume / 100),
                "large_trades_count": int(buy_volume / 10000),
                "timestamp": timestamp
            }
        }
    }


def create_whale_event(
    token_address: str,
    action: str,
    amount_usd: float,
) -> Dict[str, Any]:
    """Create a WhaleEvent wrapped in Event::Market format."""
    timestamp = datetime.now(timezone.utc).isoformat()
    
    return {
        "Market": {
            "Whale": {
                "token_address": token_address,
                "wallet_address": "DummyWhale1111111111111111111111111111111",
                "action": action,
                "amount_usd": amount_usd,
                "amount_tokens": amount_usd / 0.001,  # Assume $0.001 per token
                "wallet_balance_usd": amount_usd * 10,
                "timestamp": timestamp
            }
        }
    }


def create_social_event(
    token_address: str,
    sentiment: float,
    engagement_score: float,
    mentions: int,
) -> Dict[str, Any]:
    """Create a SocialMention event wrapped in Event::Market format."""
    timestamp = datetime.now(timezone.utc).isoformat()
    
    return {
        "Market": {
            "Social": {
                "token_address": token_address,
                "source": "twitter",
                "sentiment": sentiment,
                "engagement_score": engagement_score,
                "influencer_score": 0.8,
                "mentions_1h": mentions,
                "timestamp": timestamp
            }
        }
    }


def inject_event(client, stream: str, event: Dict[str, Any]) -> str:
    """Inject an event into Redis stream."""
    payload = json.dumps(event)
    event_type = list(event.get("Market", {}).keys())[0].lower() if "Market" in event else "unknown"
    
    # Add to stream with the expected format: type + data
    message_id = client.xadd(
        stream,
        {"type": event_type, "data": payload}
    )
    
    return message_id.decode() if isinstance(message_id, bytes) else str(message_id)


def scenario_momentum_spike(client, redis_url: str, token: str, count: int = 10):
    """
    Inject a sequence of price events simulating a momentum spike.
    This should trigger the Momentum5m strategy.
    
    Conditions for Momentum5m signal:
    - price_change > threshold (default 0.05 = 5%)
    - volume_ratio > vol_multiplier (default 2.0x)
    - liquidity > min_liquidity_usd (default 50,000)
    """
    print(f"\n🚀 Injecting MOMENTUM SPIKE scenario for {token}")
    print(f"   This should trigger Momentum5m strategy if lookback is met\n")
    
    base_price = 0.001  # Starting price
    base_volume = 50000  # Base volume
    liquidity = 100000  # Above min_liquidity_usd threshold
    
    for i in range(count):
        # Gradual price increase over the sequence
        # Final price should be 10% higher than start
        progress = (i + 1) / count
        price = base_price * (1 + 0.10 * progress)  # 10% total increase
        
        # Volume spikes in the middle of the sequence
        if i >= count // 2:
            volume = base_volume * 3.0  # 3x volume surge
        else:
            volume = base_volume
        
        price_change = 0.10 * progress  # Progressive price change
        
        event = create_price_event(
            token_address=token,
            price_usd=price,
            volume_5m=volume,
            liquidity_usd=liquidity,
            price_change_5m=price_change
        )
        
        msg_id = inject_event(client, "events:price", event)
        
        indicator = "📈" if i >= count // 2 else "📊"
        print(f"   {indicator} Event {i+1}/{count}: price=${price:.6f} (+{price_change*100:.1f}%), "
              f"vol=${volume:,.0f}, msg_id={msg_id}")
        
        time.sleep(0.5)  # Small delay between events
    
    print(f"\n✅ Injected {count} price events for momentum spike scenario")


def scenario_volume_surge(client, redis_url: str, token: str, count: int = 5):
    """
    Inject volume spike events to test volume-based strategies.
    """
    print(f"\n📊 Injecting VOLUME SURGE scenario for {token}")
    
    for i in range(count):
        # Increasing volume spike ratio
        spike_ratio = 2.0 + (i * 0.5)  # 2x, 2.5x, 3x, 3.5x, 4x
        buy_volume = 100000 * spike_ratio
        sell_volume = 50000 * spike_ratio
        
        event = create_volume_event(
            token_address=token,
            volume_spike_ratio=spike_ratio,
            buy_volume=buy_volume,
            sell_volume=sell_volume
        )
        
        # Note: Volume events are sent to events:price because the executor's
        # MasterExecutor reads from events:price and dispatches to strategies
        # based on the MarketEvent variant (Price, Volume, etc.) inside the Event wrapper.
        # The stream name is for routing, not for filtering by event type.
        msg_id = inject_event(client, "events:price", event)
        print(f"   📊 Volume spike {i+1}/{count}: ratio={spike_ratio:.1f}x, "
              f"buy=${buy_volume:,.0f}, msg_id={msg_id}")
        
        time.sleep(0.5)
    
    print(f"\n✅ Injected {count} volume spike events")


def scenario_whale_buy(client, redis_url: str, token: str, count: int = 3):
    """
    Inject whale buy events to test whale-tracking strategies.
    """
    print(f"\n🐋 Injecting WHALE BUY scenario for {token}")
    
    amounts = [50000, 100000, 250000]  # Escalating whale buys
    
    for i, amount in enumerate(amounts[:count]):
        event = create_whale_event(
            token_address=token,
            action="buy",
            amount_usd=amount
        )
        
        msg_id = inject_event(client, "events:whale", event)
        print(f"   🐋 Whale buy {i+1}/{count}: ${amount:,.0f}, msg_id={msg_id}")
        
        time.sleep(0.5)
    
    print(f"\n✅ Injected {count} whale buy events")


def scenario_social_buzz(client, redis_url: str, token: str, count: int = 5):
    """
    Inject social sentiment events to test social strategies.
    """
    print(f"\n🔥 Injecting SOCIAL BUZZ scenario for {token}")
    
    for i in range(count):
        # Increasing positive sentiment and engagement
        sentiment = 0.5 + (i * 0.1)  # 0.5 to 0.9
        engagement = 0.3 + (i * 0.15)  # 0.3 to 0.9
        mentions = 100 * (i + 1)  # 100, 200, 300, 400, 500
        
        event = create_social_event(
            token_address=token,
            sentiment=min(sentiment, 1.0),
            engagement_score=min(engagement, 1.0),
            mentions=mentions
        )
        
        msg_id = inject_event(client, "events:social", event)
        print(f"   🔥 Social {i+1}/{count}: sentiment={sentiment:.1f}, "
              f"engagement={engagement:.1f}, mentions={mentions}, msg_id={msg_id}")
        
        time.sleep(0.5)
    
    print(f"\n✅ Injected {count} social buzz events")


def scenario_perfect_trade(client, redis_url: str, token: str):
    """
    The "Perfect Trade" scenario - combines all bullish signals:
    - Momentum price spike
    - Volume surge
    - Whale accumulation
    - Positive social sentiment
    
    This should trigger multiple strategies simultaneously.
    """
    print(f"\n⭐ Injecting PERFECT TRADE scenario for {token}")
    print("   This combines momentum + volume + whale + social signals\n")
    
    # Build up history first (momentum needs lookback)
    print("   Phase 1: Building price history (5 events)...")
    for i in range(5):
        event = create_price_event(
            token_address=token,
            price_usd=0.001 + (i * 0.00002),  # Slow initial rise
            volume_5m=50000,
            liquidity_usd=100000,
            price_change_5m=0.01 * (i + 1)
        )
        inject_event(client, "events:price", event)
        time.sleep(0.3)
    
    print("   Phase 2: Whale accumulation signal...")
    whale_event = create_whale_event(token, "buy", 150000)
    inject_event(client, "events:whale", whale_event)
    time.sleep(0.3)
    
    print("   Phase 3: Social buzz eruption...")
    social_event = create_social_event(token, 0.9, 0.85, 500)
    inject_event(client, "events:social", social_event)
    time.sleep(0.3)
    
    print("   Phase 4: EXPLOSIVE price/volume spike...")
    for i in range(5):
        # Strong price increase with massive volume
        price = 0.001 * (1.0 + 0.02 * (i + 6))  # Continue from history
        volume = 50000 * (2.0 + i)  # 2x to 6x volume
        price_change = 0.08 + (i * 0.02)  # 8% to 16% change
        
        event = create_price_event(
            token_address=token,
            price_usd=price,
            volume_5m=volume,
            liquidity_usd=100000,
            price_change_5m=price_change
        )
        msg_id = inject_event(client, "events:price", event)
        print(f"      💥 Spike {i+1}/5: price=${price:.6f} (+{price_change*100:.0f}%), "
              f"vol=${volume:,.0f} ({volume/50000:.1f}x)")
        time.sleep(0.3)
    
    print("\n⭐ PERFECT TRADE scenario complete!")
    print("   Expected triggers: Momentum5m, SocialBuzz, WhaleTracker")


def custom_injection(
    client,
    token: str,
    price: float,
    volume: float,
    liquidity: float,
    price_change: float,
    count: int
):
    """Inject custom price events with user-specified parameters."""
    print(f"\n🔧 Injecting CUSTOM events for {token}")
    print(f"   Price: ${price}, Volume: ${volume:,.0f}, Liquidity: ${liquidity:,.0f}")
    print(f"   Price Change: {price_change*100:.1f}%\n")
    
    for i in range(count):
        event = create_price_event(
            token_address=token,
            price_usd=price,
            volume_5m=volume,
            liquidity_usd=liquidity,
            price_change_5m=price_change
        )
        
        msg_id = inject_event(client, "events:price", event)
        print(f"   📤 Event {i+1}/{count}: msg_id={msg_id}")
        time.sleep(0.3)
    
    print(f"\n✅ Injected {count} custom events")


def main():
    parser = argparse.ArgumentParser(description="MemeSnipe v25 Mock Event Injector")
    parser.add_argument("--scenario", type=str, 
                       choices=["momentum_spike", "volume_surge", "whale_buy", 
                               "social_buzz", "perfect_trade"],
                       help="Pre-defined test scenario to inject")
    parser.add_argument("--custom", action="store_true",
                       help="Inject custom events with specified parameters")
    parser.add_argument("--token", type=str, 
                       default="TestToken11111111111111111111111111111111",
                       help="Token address to use for events")
    parser.add_argument("--price", type=float, default=0.001,
                       help="Price USD for custom events")
    parser.add_argument("--volume", type=float, default=100000,
                       help="Volume USD (5min) for custom events")
    parser.add_argument("--liquidity", type=float, default=100000,
                       help="Liquidity USD for custom events")
    parser.add_argument("--price-change", type=float, default=0.08,
                       help="Price change (0.08 = 8%%) for custom events")
    parser.add_argument("--count", type=int, default=10,
                       help="Number of events to inject")
    parser.add_argument("--redis-url", type=str,
                       default=os.environ.get("REDIS_URL", "redis://localhost:6379"),
                       help="Redis URL")
    parser.add_argument("--env-file", type=str, default=".env",
                       help="Path to .env file to load")
    
    args = parser.parse_args()
    
    # Load environment from file if it exists
    if os.path.exists(args.env_file):
        with open(args.env_file) as f:
            for line in f:
                line = line.strip()
                if line and not line.startswith("#") and "=" in line:
                    key, _, value = line.partition("=")
                    if key not in os.environ:
                        os.environ[key] = value
    
    # Update redis_url from env if not explicitly set
    if args.redis_url == "redis://localhost:6379":
        args.redis_url = os.environ.get("REDIS_URL", args.redis_url)
    
    print("=" * 60)
    print("🧪 MemeSnipe v25 Mock Event Injector")
    print("=" * 60)
    print(f"Redis URL: {args.redis_url}")
    print(f"Token: {args.token}")
    print(f"Time: {datetime.now(timezone.utc).isoformat()}")
    
    # Connect to Redis
    try:
        client = get_redis_client(args.redis_url)
        client.ping()
        print("✅ Connected to Redis")
    except Exception as e:
        print(f"❌ Failed to connect to Redis: {e}")
        sys.exit(1)
    
    # Run the appropriate injection
    if args.custom:
        custom_injection(
            client,
            args.token,
            args.price,
            args.volume,
            args.liquidity,
            args.price_change,
            args.count
        )
    elif args.scenario == "momentum_spike":
        scenario_momentum_spike(client, args.redis_url, args.token, args.count)
    elif args.scenario == "volume_surge":
        scenario_volume_surge(client, args.redis_url, args.token, args.count)
    elif args.scenario == "whale_buy":
        scenario_whale_buy(client, args.redis_url, args.token, args.count)
    elif args.scenario == "social_buzz":
        scenario_social_buzz(client, args.redis_url, args.token, args.count)
    elif args.scenario == "perfect_trade":
        scenario_perfect_trade(client, args.redis_url, args.token)
    else:
        print("\n⚠️  No scenario specified. Use --scenario or --custom")
        print("\nAvailable scenarios:")
        print("  --scenario momentum_spike  : Simulate a momentum breakout")
        print("  --scenario volume_surge    : Simulate volume spike")
        print("  --scenario whale_buy       : Simulate whale accumulation")
        print("  --scenario social_buzz     : Simulate social media buzz")
        print("  --scenario perfect_trade   : All bullish signals combined")
        print("\nCustom injection:")
        print("  --custom --price 0.5 --volume 1000000 --liquidity 500000")
        sys.exit(0)
    
    print("\n" + "=" * 60)
    print("📋 Next Steps:")
    print("   1. Check executor logs: docker compose logs -f executor")
    print("   2. Check Redis stream length: docker compose exec redis redis-cli XLEN events:price")
    print("   3. Look for 'Strategy generated action' or 'MOMENTUM BUY signal' in logs")
    print("=" * 60)


if __name__ == "__main__":
    main()
