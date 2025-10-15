#!/usr/bin/env python3
"""Produce a lightweight market snapshot for Codex before each autonomous run.

The script pulls recent Coinbase spot data for a selected basket, computes
intraday/24h returns plus simple realized volatility, and emits a Markdown
summary file at `context/market_context.md`.
"""

from __future__ import annotations

import json
import math
import os
import pathlib
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from typing import Dict, Iterable, List
from urllib.error import URLError
from urllib.request import Request, urlopen

# --- Configuration -----------------------------------------------------------------
PRODUCTS = [
    "BTC-USD",
    "ETH-USD",
    "SOL-USD",
    "DOGE-USD",
    "WIF-USD",
]
GRANULARITY_SECONDS = 900  # 15 minute candles
MAX_PER_REQUEST = 300  # Coinbase API limit
HOURS_LOOKBACK = 24

OUTPUT_PATH = pathlib.Path("context/market_context.md")
OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)

HEADERS = {"User-Agent": "MemeSnipe-Autonomous-Context/1.0"}
BASE_URL = "https://api.exchange.coinbase.com/products/{product}/candles"


@dataclass
class Candle:
    timestamp: datetime
    low: float
    high: float
    open: float
    close: float
    volume: float

    @classmethod
    def from_api(cls, raw: List[float]) -> "Candle":
        ts = datetime.fromtimestamp(raw[0], tz=timezone.utc)
        return cls(timestamp=ts, low=raw[1], high=raw[2], open=raw[3], close=raw[4], volume=raw[5])


def chunked_candles(product: str, start: datetime, end: datetime) -> List[Candle]:
    candles: List[Candle] = []
    cursor = start
    step = GRANULARITY_SECONDS * MAX_PER_REQUEST
    while cursor < end:
        chunk_end = min(cursor + timedelta(seconds=step), end)
        url = BASE_URL.format(product=product)
        query = f"?granularity={GRANULARITY_SECONDS}&start={int(cursor.timestamp())}&end={int(chunk_end.timestamp())}"
        req = Request(url + query, headers=HEADERS)
        try:
            with urlopen(req, timeout=10) as resp:
                payload = json.loads(resp.read().decode())
        except URLError as exc:  # pragma: no cover - network/environment failure
            print(f"Warning: failed fetching {product} candles: {exc}", file=sys.stderr)
            break

        candles.extend(Candle.from_api(row) for row in payload)
        cursor = chunk_end
        time.sleep(0.2)  # avoid rate limits

    candles.sort(key=lambda c: c.timestamp)
    return candles


def realized_vol(closes: Iterable[float]) -> float:
    closes = list(closes)
    if len(closes) < 2:
        return float("nan")
    log_returns = [math.log(closes[i] / closes[i - 1]) for i in range(1, len(closes)) if closes[i - 1] > 0]
    if not log_returns:
        return float("nan")
    mean = sum(log_returns) / len(log_returns)
    variance = sum((r - mean) ** 2 for r in log_returns) / len(log_returns)
    return math.sqrt(variance) * math.sqrt(365 * 24 * 60 * 60 / GRANULARITY_SECONDS)


def summarize_product(product: str, candles: List[Candle]) -> Dict[str, float]:
    if not candles:
        return {
            "last_price": float("nan"),
            "return_1h": float("nan"),
            "return_24h": float("nan"),
            "realized_vol_24h": float("nan"),
            "volume_24h": float("nan"),
        }

    closes = [c.close for c in candles]
    last_price = closes[-1]

    def pct_change(period: timedelta) -> float:
        cutoff = candles[-1].timestamp - period
        ref = next((c.close for c in candles if c.timestamp >= cutoff), closes[0])
        if ref == 0:
            return float("nan")
        return (last_price / ref) - 1

    vol_24h = realized_vol(closes[-int(HOURS_LOOKBACK * 3600 / GRANULARITY_SECONDS) :])
    vol_sum = sum(c.volume for c in candles[-int(HOURS_LOOKBACK * 3600 / GRANULARITY_SECONDS) :])

    return {
        "last_price": last_price,
        "return_1h": pct_change(timedelta(hours=1)),
        "return_24h": pct_change(timedelta(hours=24)),
        "realized_vol_24h": vol_24h,
        "volume_24h": vol_sum,
    }


def format_pct(value: float) -> str:
    if math.isnan(value):
        return "n/a"
    return f"{value * 100:+.2f}%"


def format_vol(value: float) -> str:
    if math.isnan(value):
        return "n/a"
    return f"{value:.2f}"


def format_number(value: float) -> str:
    if math.isnan(value):
        return "n/a"
    if value >= 1_000_000:
        return f"{value/1_000_000:.2f}M"
    if value >= 1_000:
        return f"{value/1_000:.2f}K"
    return f"{value:.2f}"


def main() -> None:
    now = datetime.now(tz=timezone.utc)
    start = now - timedelta(hours=HOURS_LOOKBACK)

    rows = []
    for product in PRODUCTS:
        candles = chunked_candles(product, start, now)
        stats = summarize_product(product, candles)
        rows.append((product, stats))

    lines = ["# Market Context Snapshot", ""]
    lines.append(f"Generated: {now.isoformat()}")
    lines.append("")
    lines.append("| Product | Last | 1h Δ | 24h Δ | 24h Realized Vol | 24h Volume |")
    lines.append("| --- | ---: | ---: | ---: | ---: | ---: |")

    for product, stats in rows:
        lines.append(
            "| {product} | {price} | {ret1h} | {ret24h} | {vol} | {volume} |".format(
                product=product,
                price=format_number(stats["last_price"]),
                ret1h=format_pct(stats["return_1h"]),
                ret24h=format_pct(stats["return_24h"]),
                vol=format_vol(stats["realized_vol_24h"]),
                volume=format_number(stats["volume_24h"]),
            )
        )

    lines.append("")
    lines.append("Notes: Coinbase hourly data, realized vol annualised from 15m log returns.")

    OUTPUT_PATH.write_text("\n".join(lines))


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # pragma: no cover - defensive logging
        print(f"Failed to generate market context: {exc}", file=sys.stderr)
        sys.exit(1)
