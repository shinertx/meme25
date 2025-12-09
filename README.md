# MemeSnipe v25: The Monolith

**Status:** ACTIVE
**Architecture:** Single Rust Binary (Zero-Latency)
**Strategy:** Kinetic Velocity Sniper (Pump.fun Migrations)

## 🚀 Overview

MemeSnipe v25 is a high-frequency trading system designed to snipe Pump.fun liquidity migrations in the same block they are created.

It has been refactored from a distributed microservices architecture into a single, atomic Rust binary to minimize latency.

## ⚡ Components

The system runs three concurrent tasks in a single process:

1.  **MarketDataGateway (The Eyes):**
    *   Monitors Solana blockchain for Pump.fun `Migrate` instructions.
    *   Program ID: `6EF8rrecthR5DkdfiS9KYQaM21wC3n6R1zb5Y5q7pump`

2.  **RugCheck (The Shield):**
    *   Filters opportunities using **Kinetic Velocity** physics.
    *   **Buy Signal:** Velocity > 2.0 SOL/sec (High Energy).
    *   **Ignore:** Velocity < 0.2 SOL/sec (Slow/Dead).

3.  **PumpSwapAtomicSniper (The Weapon):**
    *   Executes trades via **Jito Bundles**.
    *   Guarantees "First Fill" by bundling the Buy transaction immediately after the Migration transaction.

## 🛠️ Usage

### Prerequisites
*   Rust (latest stable)
*   Solana CLI tools

### Running the System

```bash
# Run in Release mode for maximum performance
cargo run -p meme_snipe_monolith --release
```

## 📂 Structure

*   `monolith/`: The core source code.
*   `Cargo.toml`: Workspace configuration.

---
*Built for Speed. Engineered for Alpha.*
