from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from typing import Dict, List, Optional, Any, Tuple
import asyncio
import pandas as pd
import numpy as np
from datetime import datetime, timedelta
from collections import OrderedDict
import redis.asyncio as redis
import httpx
import uuid
import os
import json

app = FastAPI(title="MemeSnipe Backtest Engine", version="1.0.0")

# Redis connection
redis_client = redis.from_url("redis://redis:6379", decode_responses=True)

# Birdeye API client
BIRDEYE_API_KEY = os.getenv("BIRDEYE_API_KEY", "")
BIRDEYE_API_URL = "https://public-api.birdeye.so"

# Minimal address->CoinGecko ID map for fallback (extend as needed)
COINGECKO_IDS = {
    # WIF (dogwifhat)
    "EKpQGSJtjMFqKZ9KQanSqYXRcF8fBopzL7xiH5HwM4A7": "dogwifcoin",
    # WIF canonical mint alias
    "EKpQGSJtjMFqKZ9KQanSqYXRcF8fBopzLHYxdM65zcjm": "dogwifcoin",
    # SOL (native SOL mint)
    "So11111111111111111111111111111111111111112": "solana",
    # BONK (Solana)
    "DezXAZ8z7PnrnRJjz3E2YkdY8YPh91qCP83N5dEJ9h5z": "bonk",
    # POPCAT (Solana)
    "7GCihgDB8fe6KNjn2MYtkzZcRjQy3t9GHdC8uHYmW2hr": "popcat",
    # BODEN (Solana)
    "3psH1Mj1f7yUfaD5gh6Zj7epE8hhrMkMETgv5TshQA4o": "jeo-boden",
}

COINGECKO_CACHE: OrderedDict[Tuple[str, int, int], Tuple[pd.DataFrame, datetime]] = OrderedDict()
COINGECKO_CACHE_TTL = timedelta(minutes=30)
COINGECKO_CACHE_MAX = 16

# In-memory OHLCV cache to accelerate parameter sweeps
DATA_CACHE: Dict[tuple, pd.DataFrame] = {}
DATA_CACHE_MAX = 8

async def fetch_coingecko_data(token_address: str, start_time: datetime, end_time: datetime) -> pd.DataFrame:
    """Fallback: fetch price data from CoinGecko (market_chart/range) and synthesize OHLCV.

    Notes:
    - We use the range endpoint with explicit from/to to avoid enterprise-only interval parameters.
    - CoinGecko returns uneven timestamps at ~5m granularity for short ranges; we resample to 5-minute bars.
    - We derive OHLC from tick-like price points and volume from total_volumes diffs.
    """
    token_id = COINGECKO_IDS.get(token_address)
    if not token_id:
        raise HTTPException(status_code=404, detail="Token not supported by CoinGecko fallback.")

    # Use market_chart/range with unix seconds; avoid enterprise-only interval params
    url = f"https://api.coingecko.com/api/v3/coins/{token_id}/market_chart/range"
    params = {
        "vs_currency": "usd",
        "from": int(start_time.timestamp()),
        "to": int(end_time.timestamp()),
    }

    cache_key = (token_id, params["from"], params["to"])
    now = datetime.utcnow()
    if cache_key in COINGECKO_CACHE:
        cached_df, cached_ts = COINGECKO_CACHE[cache_key]
        if now - cached_ts < COINGECKO_CACHE_TTL:
            COINGECKO_CACHE.move_to_end(cache_key)
            return cached_df.copy()
        COINGECKO_CACHE.pop(cache_key, None)

    duration_minutes = max(1, int((end_time - start_time).total_seconds() / 60))
    if duration_minutes <= 720:  # up to 12 hours -> 1 minute bars
        resample_rule = "1min"
    elif duration_minutes <= 4320:  # up to 3 days -> 5 minute bars
        resample_rule = "5min"
    else:  # longer windows -> 15 minute bars to reduce API load
        resample_rule = "15min"

    async with httpx.AsyncClient(timeout=30.0) as client:
        try:
            # Simple retry with backoff to reduce transient failures
            for attempt in range(3):
                try:
                    resp = await client.get(url, params=params)
                    resp.raise_for_status()
                    break
                except Exception as e:
                    if attempt < 2:
                        await asyncio.sleep(1.5 * (attempt + 1))
                    else:
                        raise
            data = resp.json()
            prices = data.get("prices", [])
            volumes = data.get("total_volumes", [])
            if not prices:
                raise HTTPException(status_code=404, detail="No CoinGecko price data available.")

            # Build price series
            df_p = pd.DataFrame(prices, columns=["ms", "price"])  # [ms, price]
            df_p["timestamp"] = pd.to_datetime(df_p["ms"], unit="ms")
            df_p = df_p.set_index("timestamp").drop(columns=["ms"]).sort_index()

            # Build OHLC from price with 5m resampling (safe default granularity)
            ohlc = df_p["price"].resample(resample_rule).ohlc()

            # Volume: diff of total_volumes, resampled to 5m
            if volumes:
                df_v = pd.DataFrame(volumes, columns=["ms", "total_volume"]).sort_values("ms")
                df_v["timestamp"] = pd.to_datetime(df_v["ms"], unit="ms")
                df_v = df_v.set_index("timestamp").drop(columns=["ms"]).sort_index()
                df_v["vol"] = df_v["total_volume"].diff().clip(lower=0)
                vol = df_v["vol"].resample(resample_rule).sum().fillna(0)
            else:
                vol = pd.Series(0, index=ohlc.index)

            df = pd.concat([ohlc, vol.rename("volume")], axis=1).dropna(how="any")
            # Clip to requested window
            df = df[(df.index >= start_time) & (df.index <= end_time)]
            if df.empty:
                raise HTTPException(status_code=404, detail="No data in requested window from CoinGecko.")
            result = df.rename(columns={"open": "open", "high": "high", "low": "low", "close": "close"})
            COINGECKO_CACHE[cache_key] = (result.copy(), datetime.utcnow())
            COINGECKO_CACHE.move_to_end(cache_key)
            while len(COINGECKO_CACHE) > COINGECKO_CACHE_MAX:
                COINGECKO_CACHE.popitem(last=False)
            return result
        except httpx.HTTPStatusError as e:
            raise HTTPException(status_code=e.response.status_code, detail=f"CoinGecko error: {e.response.text}")
        except Exception as e:
            raise HTTPException(status_code=500, detail=f"CoinGecko fallback failed: {str(e)}")


class BacktestRequest(BaseModel):
    strategy_id: str
    strategy_params: Dict[str, Any]
    token_address: str
    start_date: str
    end_date: str
    initial_capital: float = 200.0
    slippage_bps: int = 50  # 0.5% slippage


class BacktestResult(BaseModel):
    backtest_id: str
    strategy_id: str
    token_address: str
    start_date: str
    end_date: str
    initial_capital: float
    final_capital: float
    total_pnl: float
    total_return_pct: float
    sharpe_ratio: float
    max_drawdown: float
    win_rate: float
    total_trades: int
    avg_trade_duration_minutes: float
    trade_log: List[Dict[str, Any]]


# --- Data Fetching ---
async def fetch_historical_data(
    token_address: str, start_time: datetime, end_time: datetime
) -> pd.DataFrame:
    """Fetches historical price data from Birdeye."""
    cache_key = (token_address, int(start_time.timestamp()), int(end_time.timestamp()))
    if cache_key in DATA_CACHE:
        return DATA_CACHE[cache_key].copy()
    # Prefer Birdeye if key present
    if BIRDEYE_API_KEY:
        headers = {"X-API-KEY": BIRDEYE_API_KEY}
        url = f"{BIRDEYE_API_URL}/defi/history_price"
        params = {
            "address": token_address,
            "address_type": "token",
            "type": "1m",
            "time_from": int(start_time.timestamp()),
            "time_to": int(end_time.timestamp()),
        }
        async with httpx.AsyncClient(timeout=30.0) as client:
            try:
                response = await client.get(url, headers=headers, params=params)
                response.raise_for_status()
                data = response.json()
                if not data.get("data", {}).get("items"):
                    # Fall back if empty
                    return await fetch_coingecko_data(token_address, start_time, end_time)

                df = pd.DataFrame(data["data"]["items"])
                df["timestamp"] = pd.to_datetime(df["unixTime"], unit="s")
                df = df.set_index("timestamp")
                df = df.rename(columns={"value": "price"})
                df["price"] = df["price"].astype(float)
                # Ensure we have OHLCV data; otherwise resample or switch endpoint.
                if "open" not in df.columns:
                    df["open"] = df["price"]
                    df["high"] = df["price"]
                    df["low"] = df["price"]
                    df["close"] = df["price"]
                if "volume" not in df.columns:  # Synthesize volume if not present
                    df["volume"] = np.random.randint(1000, 10000, size=len(df))

                df = df[["open", "high", "low", "close", "volume"]].sort_index()
                # Cache store with naive eviction
                if len(DATA_CACHE) >= DATA_CACHE_MAX:
                    try:
                        DATA_CACHE.pop(next(iter(DATA_CACHE)))
                    except StopIteration:
                        pass
                DATA_CACHE[cache_key] = df
                return df
            except httpx.HTTPStatusError as e:
                # Unauthorized/Forbidden -> fallback
                if e.response.status_code in (401, 403):
                    return await fetch_coingecko_data(token_address, start_time, end_time)
                raise HTTPException(
                    status_code=e.response.status_code,
                    detail=f"Error fetching data from Birdeye: {e.response.text}",
                )
            except Exception:
                # Generic fallback
                return await fetch_coingecko_data(token_address, start_time, end_time)
    else:
        # No Birdeye key: use fallback
        df = await fetch_coingecko_data(token_address, start_time, end_time)
        if len(DATA_CACHE) >= DATA_CACHE_MAX:
            try:
                DATA_CACHE.pop(next(iter(DATA_CACHE)))
            except StopIteration:
                pass
        DATA_CACHE[cache_key] = df
        return df


# --- Performance Metrics ---
def calculate_performance_metrics(
    trade_log: List[Dict], initial_capital: float, daily_returns: pd.Series
):
    completed_trades = [t for t in trade_log if t.get("exit_time")]
    if not completed_trades:
        return {
            "final_capital": initial_capital,
            "total_pnl": 0,
            "total_return_pct": 0,
            "sharpe_ratio": 0,
            "max_drawdown": 0,
            "win_rate": 0,
            "total_trades": 0,
            "avg_trade_duration_minutes": 0,
        }

    final_capital = initial_capital + sum(t["pnl"] for t in completed_trades)
    total_pnl = final_capital - initial_capital
    total_return_pct = (total_pnl / initial_capital) * 100

    # Sharpe Ratio
    # Assuming risk-free rate is 0. Using daily returns for calculation.
    if daily_returns.empty:
        sharpe_ratio = 0.0
    else:
        # Robustness: require at least 5 observations to avoid unstable Sharpe
        if len(daily_returns) < 5:
            sharpe_ratio = 0.0
        else:
            returns_std = daily_returns.std()
            if pd.isna(returns_std) or returns_std == 0:
                sharpe_ratio = 0.0
            else:
                sharpe_ratio = (daily_returns.mean() / returns_std) * np.sqrt(
                    365
                )  # Annualized

    # Max Drawdown
    if daily_returns.empty:
        max_drawdown = 0.0
    else:
        equity_curve = (
            initial_capital + daily_returns.cumsum() * initial_capital
        )
        if equity_curve.empty:
            max_drawdown = 0.0
        else:
            peak = equity_curve.expanding(min_periods=1).max()
            drawdown = (equity_curve - peak) / peak
            if drawdown.empty:
                max_drawdown = 0.0
            else:
                drawdown_min = drawdown.min()
                max_drawdown = (
                    drawdown_min * 100 if not pd.isna(drawdown_min) else 0.0
                )

    wins = sum(1 for t in completed_trades if t["pnl"] > 0)
    win_rate = (wins / len(completed_trades)) * 100 if completed_trades else 0

    total_trades = len(completed_trades)

    durations = [
        (t["exit_time"] - t["entry_time"]).total_seconds() / 60
        for t in completed_trades
        if t.get("exit_time") and t.get("entry_time")
    ]
    avg_trade_duration_minutes = np.mean(durations) if durations else 0

    return {
        "final_capital": final_capital,
        "total_pnl": total_pnl,
        "total_return_pct": total_return_pct,
        "sharpe_ratio": sharpe_ratio,
        "max_drawdown": max_drawdown,
        "win_rate": win_rate,
        "total_trades": total_trades,
        "avg_trade_duration_minutes": avg_trade_duration_minutes,
    }


# --- Strategy Simulation ---
async def run_strategy_simulation(
    data: pd.DataFrame,
    strategy_id: str,
    strategy_params: Dict,
    initial_capital: float,
    slippage_bps: int,
) -> List[Dict]:
    """
    Simulates the 'volume_spike' trading strategy.

    Strategy Logic:
    1. Calculate a rolling average of trading volume.
    2. Treat current volume above multiplier * average as a spike.
    3. Buy on the spike.
    4. Exit after `holding_period_minutes`.
    """
    if strategy_id not in ("volume_spike", "price_momentum"):
        raise HTTPException(status_code=400, detail=f"Strategy '{strategy_id}' not supported.")

    # --- Parameter setup & indicators ---
    trade_size_usd = strategy_params.get("trade_size_usd", 100)
    # Infer bar length
    if len(data.index) >= 2:
        inferred_minutes = max(1, int((data.index[1] - data.index[0]).total_seconds() // 60) or 1)
    else:
        inferred_minutes = 5

    if strategy_id == "volume_spike":
        lookback_period = strategy_params.get("lookback_period", 60)  # minutes
        volume_multiplier = strategy_params.get("volume_multiplier", 10)
        holding_period_minutes = strategy_params.get("holding_period_minutes", 30)
        window_bars = max(1, int(round(lookback_period / inferred_minutes)))
        data["volume_ma"] = data["volume"].rolling(window=window_bars).mean()
    else:  # price_momentum
        fast_minutes = strategy_params.get("fast_minutes", 30)
        slow_minutes = strategy_params.get("slow_minutes", 120)
        holding_period_minutes = strategy_params.get("max_holding_minutes", 240)
        min_gap_bps = float(strategy_params.get("min_crossover_gap_bps", 0))
        # Regime filter & ATR sizing
        regime_sma_minutes = int(strategy_params.get("regime_sma_minutes", 0))
        atr_minutes = int(strategy_params.get("atr_minutes", 0))
        risk_per_trade_pct = float(strategy_params.get("risk_per_trade_pct", 0))
        fast_span = max(1, int(round(fast_minutes / inferred_minutes)))
        slow_span = max(2, int(round(slow_minutes / inferred_minutes)))
        data["ema_fast"] = data["close"].ewm(span=fast_span, adjust=False).mean()
        data["ema_slow"] = data["close"].ewm(span=slow_span, adjust=False).mean()

        if regime_sma_minutes > 0:
            sma_span = max(1, int(round(regime_sma_minutes / inferred_minutes)))
            data["sma_regime"] = data["close"].rolling(window=sma_span).mean()
        else:
            data["sma_regime"] = np.nan

        if atr_minutes > 0:
            atr_span = max(1, int(round(atr_minutes / inferred_minutes)))
            # True Range components
            close_prev = data["close"].shift(1)
            tr1 = data["high"] - data["low"]
            tr2 = (data["high"] - close_prev).abs()
            tr3 = (data["low"] - close_prev).abs()
            tr = pd.concat([tr1, tr2, tr3], axis=1).max(axis=1)
            data["atr"] = tr.rolling(window=atr_span).mean()
        else:
            data["atr"] = np.nan

    trade_log: List[Dict[str, Any]] = []
    in_position = False
    position_entry_time = None  # use pandas Timestamp during simulation
    position_entry_price = 0.0

    # Risk controls: optional SL/TP in bps from entry
    stop_loss_bps = float(strategy_params.get("stop_loss_bps", 0))
    take_profit_bps = float(strategy_params.get("take_profit_bps", 0))

    # Ensure datetime index
    if not isinstance(data.index, pd.DatetimeIndex):
        data.index = pd.to_datetime(data.index)

    for i in range(len(data.index)):
        current_time = data.index[i]  # pandas Timestamp
        row = data.iloc[i]
        current_time_py = current_time.to_pydatetime()

        # Check for exit condition first (time-based)
        if (
            in_position
            and position_entry_time is not None
            and (
                current_time
                >= position_entry_time + pd.Timedelta(minutes=holding_period_minutes)
            )
        ):
            exit_price = row["open"] * (1 - slippage_bps / 10000)  # slippage
            if position_entry_price <= 0:
                in_position = False
                position_entry_time = None
                position_entry_price = 0.0
            else:
                pnl = (exit_price - position_entry_price) * (
                    trade_size_usd / position_entry_price
                )
                for trade in reversed(trade_log):
                    if trade["exit_time"] is None:
                        trade["exit_time"] = current_time_py
                        trade["exit_price"] = exit_price
                        trade["pnl"] = pnl
                        break
            in_position = False
            position_entry_time = None
            position_entry_price = 0.0

        # Intrabar SL/TP exits using high/low after time exit check
        if in_position and position_entry_price > 0:
            # compute thresholds
            stop_price = position_entry_price * (1 - stop_loss_bps / 10000) if stop_loss_bps > 0 else None
            tp_price = position_entry_price * (1 + take_profit_bps / 10000) if take_profit_bps > 0 else None
            # trigger flags
            sl_hit = stop_price is not None and float(row.get("low", row["open"])) <= stop_price
            tp_hit = tp_price is not None and float(row.get("high", row["open"])) >= tp_price
            hit = None
            if sl_hit and tp_hit:
                # prioritize whichever is closer to open as conservative fill assumption
                open_px = float(row["open"])
                # Guard and cast
                if stop_price is not None and tp_price is not None:
                    sl_slip = abs(open_px - float(stop_price))
                    tp_slip = abs(float(tp_price) - open_px)
                else:
                    # Fallback if any is None (shouldn't happen due to sl_hit/tp_hit guards)
                    sl_slip = float('inf') if stop_price is None else abs(open_px - float(stop_price))
                    tp_slip = float('inf') if tp_price is None else abs(float(tp_price) - open_px)
                hit = "tp" if tp_slip < sl_slip else "sl"
            elif sl_hit:
                hit = "sl"
            elif tp_hit:
                hit = "tp"
            if hit:
                px_opt = (tp_price if hit == "tp" else stop_price)
                # Fallback to open if None (shouldn't happen due to hit checks)
                if px_opt is None:
                    px_opt = float(row["open"])
                px = float(px_opt) * (1 - slippage_bps / 10000)
                pnl = (px - position_entry_price) * (trade_size_usd / position_entry_price)
                for trade in reversed(trade_log):
                    if trade["exit_time"] is None:
                        trade["exit_time"] = current_time_py
                        trade["exit_price"] = px
                        trade["pnl"] = pnl
                        break
                in_position = False
                position_entry_time = None
                position_entry_price = 0.0

        # Check for entry condition
        if not in_position:
            if strategy_id == "volume_spike":
                cond = pd.notna(row["volume_ma"]) and row["volume"] > (row["volume_ma"] * volume_multiplier)
            else:
                if i == 0 or pd.isna(row["ema_fast"]) or pd.isna(row["ema_slow"]):
                    cond = False
                else:
                    prev = data.iloc[i - 1]
                    # Enforce a minimum gap to avoid micro-cross noise
                    gap_ok = True
                    if "min_crossover_gap_bps" in strategy_params and min_gap_bps > 0:
                        # gap in bps relative to slow EMA
                        slow = float(row["ema_slow"])
                        fast = float(row["ema_fast"])
                        if slow > 0:
                            gap_bps = (fast - slow) / slow * 10000.0
                            gap_ok = gap_bps >= min_gap_bps
                    # Optional regime: only take longs if price above SMA
                    regime_ok = True
                    if regime_sma_minutes > 0 and not pd.isna(row.get("sma_regime", np.nan)):
                        regime_ok = float(row["open"]) >= float(row["sma_regime"])

                    cond = (
                        (prev["ema_fast"] <= prev["ema_slow"]) and
                        (row["ema_fast"] > row["ema_slow"]) and
                        gap_ok and
                        regime_ok and
                        (row["open"] > 0)
                    )

            if cond:
                entry_price = row["open"] * (1 + slippage_bps / 10000)
                if entry_price > 0:
                    total_realized_pnl = sum(t["pnl"] for t in trade_log if t["exit_time"] is not None)
                    current_capital = initial_capital + total_realized_pnl
                    this_trade_size = trade_size_usd
                    # ATR-aware sizing if requested and ATR available: risk_per_trade_pct of capital / (ATR distance)
                    if risk_per_trade_pct > 0 and not pd.isna(row.get("atr", np.nan)):
                        atr = float(row["atr"]) if not pd.isna(row["atr"]) else 0.0
                        if atr > 0:
                            risk_usd = max(1e-6, current_capital * (risk_per_trade_pct / 100.0))
                            # assume stop at entry - ATR
                            per_unit_risk = atr
                            units = risk_usd / per_unit_risk
                            this_trade_size = max(1.0, units * entry_price)
                    if current_capital >= this_trade_size:
                        in_position = True
                        position_entry_time = current_time
                        position_entry_price = entry_price
                        trade_log.append(
                            {
                                "entry_time": current_time_py,
                                "exit_time": None,
                                "entry_price": position_entry_price,
                                "exit_price": None,
                                "pnl": 0.0,
                            }
                        )

    # If still in position at the end, close it with the last known price
    if in_position:
        last_price = data["close"].iloc[-1]
        exit_price = last_price * (1 - slippage_bps / 10000)
        if position_entry_price > 0:
            pnl = (exit_price - position_entry_price) * (
                trade_size_usd / position_entry_price
            )
            for trade in reversed(trade_log):
                if trade["exit_time"] is None:
                    last_ts = data.index[-1]
                    trade["exit_time"] = last_ts.to_pydatetime()
                    trade["exit_price"] = exit_price
                    trade["pnl"] = pnl
                    break

    return trade_log


# --- Main Endpoint ---
@app.post("/backtest", response_model=BacktestResult)
async def run_backtest(request: BacktestRequest):
    """
    Runs a backtest for a given strategy, token, and time period.
    """
    backtest_id = f"bt_{uuid.uuid4()}"

    try:
        start_date = datetime.fromisoformat(request.start_date)
        end_date = datetime.fromisoformat(request.end_date)
    except ValueError:
        raise HTTPException(
            status_code=400,
            detail=(
                "Invalid date format; expected YYYY-MM-DDTHH:MM:SS "
                "(ISO 8601)."
            ),
        )

    # Fetch data
    data = await fetch_historical_data(
        request.token_address, start_date, end_date
    )

    # Simulate
    trade_log = await run_strategy_simulation(
        data,
        request.strategy_id,
        request.strategy_params,
        request.initial_capital,
        request.slippage_bps,
    )

    # Calculate portfolio returns for metrics
    # Build daily returns from closed trades PnL
    closed_trades = [t for t in trade_log if t.get("exit_time") is not None]
    if closed_trades:
        df_trades = pd.DataFrame(closed_trades)
        if "exit_time" in df_trades.columns and not df_trades["exit_time"].isna().all():
            exits = pd.to_datetime(df_trades["exit_time"], errors="coerce")
            df_trades = df_trades.assign(exit_time=exits).dropna(subset=["exit_time"]).copy()
            if not df_trades.empty:
                df_trades = df_trades.set_index("exit_time").sort_index()
                daily_pnl = df_trades["pnl"].resample("D").sum()
                daily_returns = daily_pnl / request.initial_capital
            else:
                daily_returns = pd.Series(dtype=float)
        else:
            daily_returns = pd.Series(dtype=float)
    else:
        daily_returns = pd.Series(dtype=float)

    # Calculate performance
    performance = calculate_performance_metrics(
        trade_log, request.initial_capital, daily_returns
    )

    result = BacktestResult(
        backtest_id=backtest_id,
        strategy_id=request.strategy_id,
        token_address=request.token_address,
        start_date=request.start_date,
        end_date=request.end_date,
        initial_capital=request.initial_capital,
        trade_log=trade_log,
        **performance,
    )

    # Optionally, store result in Redis
    try:
        # Pydantic v2 serialization
        payload = result.model_dump_json()
        await redis_client.set(
            f"backtest_result:{backtest_id}", payload, ex=timedelta(hours=24)
        )

        summary = {
            "backtest_id": backtest_id,
            "strategy_id": result.strategy_id,
            "token_address": result.token_address,
            "sharpe_ratio": result.sharpe_ratio,
            "total_return_pct": result.total_return_pct,
            "max_drawdown": result.max_drawdown,
            "total_trades": result.total_trades,
            "timestamp": datetime.utcnow().isoformat(),
        }
        await redis_client.xadd(
            "backtest_results",
            {"result": json.dumps(summary)},
        )
    except Exception:
        # Best-effort: do not fail the request if Redis is unavailable
        pass

    return result


@app.get("/health")
async def health() -> Dict[str, Any]:
    """Lightweight health endpoint with optional Redis check."""
    redis_ok = False
    try:
        pong = await redis_client.ping()
        redis_ok = bool(pong)
    except Exception:
        redis_ok = False
    return {"status": "ok", "redis": redis_ok}
