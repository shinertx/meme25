# 🚧 Pre-Production Blockers & Roadmap

The following critical issues must be resolved before the system can be considered "Production Ready" for live trading.

## 1. Pending-Bundle Awareness (CRITICAL)
- **Issue**: `JitoExecutor.executeAndConfirm` returns a `bundleId` immediately after submission, but `SniperEngine` treats this as a confirmed buy.
- **Risk**: If the bundle is dropped or rejected, the bot will track a "ghost position" and attempt to sell tokens it doesn't own, leading to errors and potential state corruption.
- **Fix**: 
  - Update `JitoExecutor` to return a `Promise<boolean>` that resolves only when the bundle is `confirmed` or `finalized`.
  - Update `SniperEngine` to await this confirmation before calling `recordPosition`.

## 2. Price & P&L Tracking (CRITICAL)
- **Issue**: `SniperEngine` hardcodes `entryPrice` to `1.0` (Line 138).
- **Risk**: P&L calculations (`(current - entry) / entry`) will be garbage. Stop-loss and Take-profit logic will fail.
- **Fix**: 
  - Fetch the real price from Raydium (using `getPoolPrice`) immediately after a confirmed buy.
  - Pass this real price to `recordPosition`.

## 3. Supply & Cabal Check Accuracy (HIGH)
- **Issue**: `MigrationListener` assumes a fixed supply of `1,000,000,000` (1B) with 6 decimals for all tokens.
- **Risk**: Non-standard tokens (burns, different decimals) will have incorrect "Insider %" calculations, leading to false positives (blocking good tokens) or false negatives (buying rugs).
- **Fix**: 
  - Fetch the actual Mint Supply using `connection.getTokenSupply(mint)`.
  - Use the real supply for the percentage calculation.

## 4. Operational Monitoring (MEDIUM)
- **Issue**: No automated health checks or alerting.
- **Risk**: If the process hangs or the RPC rate-limits (429) indefinitely, the bot stops working without notification.
- **Fix**: 
  - Implement a "Heartbeat" log every 5 minutes.
  - Add a simple Discord/Telegram webhook for critical errors (Buy/Sell execution failures).

## 5. Simulation Gaps (RESOLVED)
- **Status**: ✅ Fixed.
- **Note**: The `simulate_migration.ts` script now correctly mocks Vault accounts, allowing `calculateAmountOut` to work. However, running a "dry run" inside the main bot logic (without the script) still relies on live data which might be empty for new pools.

---

## 🗓️ Action Plan

1.  **Fix Supply Check**: Update `MigrationListener.ts` to fetch real supply.
2.  **Fix Price Tracking**: Update `SniperEngine.ts` to fetch price after buy.
3.  **Fix Bundle Confirmation**: Refactor `JitoExecutor` to wait for confirmation.
