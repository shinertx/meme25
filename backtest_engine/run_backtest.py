import httpx
import asyncio
import json
from datetime import datetime, timedelta


async def main():
    """
    Client to run a backtest against the running backtest engine service.
    """
    # --- Backtest Parameters ---
    # We'll use a recent, volatile token for this test.
    # WIF (dogwifhat) is a good candidate. Address on Solana.
    token_address = "EKpQGSJtjMFqKZ9KQanSqYXRcF8fBopzL7xiH5HwM4A7"

    # Test over a recent 7-day period
    end_date = datetime.now()
    start_date = end_date - timedelta(days=7)

    request_payload = {
        "strategy_id": "volume_spike",
        "strategy_params": {
            "lookback_period": 60,  # 60-minute rolling volume average
            "volume_multiplier": 15,  # Require 15x avg volume
            "holding_period_minutes": 120,  # Hold for 2 hours
            "trade_size_usd": 100,  # Use $100 per trade
        },
        "token_address": token_address,
        "start_date": start_date.isoformat(),
        "end_date": end_date.isoformat(),
        "initial_capital": 200.0,
        "slippage_bps": 50,  # 0.5%
    }

    print("🚀 Starting backtest with the following parameters:")
    print(json.dumps(request_payload, indent=2))

    async with httpx.AsyncClient(timeout=300.0) as client:
        try:
            response = await client.post(
                "http://localhost:8000/backtest", json=request_payload
            )
            response.raise_for_status()

            result = response.json()

            print("\n✅ Backtest Completed Successfully!")
            print("-" * 30)
            print(
                "📈 Performance Results for "
                f"'{result['strategy_id']}' on token {result['token_address']}"
            )
            print("-" * 30)
            print(f"Initial Capital: ${result['initial_capital']:.2f}")
            print(f"Final Capital:   ${result['final_capital']:.2f}")
            print(f"Total PnL:       ${result['total_pnl']:.2f}")
            print(f"Total Return:    {result['total_return_pct']:.2f}%")
            print(f"Sharpe Ratio:    {result['sharpe_ratio']:.2f}")
            print(f"Max Drawdown:    {result['max_drawdown']:.2f}%")
            print(f"Win Rate:        {result['win_rate']:.2f}%")
            print(f"Total Trades:    {result['total_trades']}")
            print(
                "Avg. Duration:   "
                f"{result['avg_trade_duration_minutes']:.2f} min"
            )
            print("-" * 30)

            # Basic check against our performance standards
            if result["sharpe_ratio"] >= 1.5 and result["max_drawdown"] > -10:
                print("\n🎉 Strategy meets minimum performance criteria!")
            else:
                print(
                    "\n⚠️ Strategy does NOT meet minimum performance criteria."
                )
                if result["sharpe_ratio"] < 1.5:
                    print(
                        "   - Sharpe Ratio "
                        f"is {result['sharpe_ratio']:.2f} (Target: >= 1.5)"
                    )
                if result["max_drawdown"] <= -10:
                    print(
                        "   - Max Drawdown "
                        f"is {result['max_drawdown']:.2f}% (Target: > -10%)"
                    )

        except httpx.HTTPStatusError as e:
            print(f"\n❌ Error running backtest: {e.response.status_code}")
            print(f"   Detail: {e.response.text}")
        except Exception as e:
            print(f"\n❌ An unexpected error occurred: {str(e)}")


if __name__ == "__main__":
    asyncio.run(main())
