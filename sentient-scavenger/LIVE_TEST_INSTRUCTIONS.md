# 🚀 Live Smoke Test Instructions

The system has passed all mock tests and audits. It is time for a real-money smoke test.

## ⚠️ Safety First
- **Risk**: Real SOL will be spent.
- **Goal**: Verify the bot can Buy, Track, and Sell on mainnet without crashing.
- **Budget**: We will use a tiny wager (0.001 SOL) to minimize risk.

## 1. Configuration Setup

### A. Edit `src/config.ts`
Open `src/config.ts` and ensure these values are set:

```typescript
// Risk per trade (in SOL) - SET TO TINY AMOUNT
export const SOL_WAGER_AMOUNT = 0.001; 

// Max tip to Jito (in SOL) - Keep low
export const JITO_TIP_CAP = 0.001; 

// Polling Intervals (Safety)
export const BLOCKHASH_POLL_INTERVAL = 2000;
export const PRICE_POLL_INTERVAL = 5000;
```

### B. Edit `.env`
Open `.env` and disable Dry Run:

```bash
DRY_RUN=false
```

## 2. Execution

Run the bot in a terminal:

```bash
npm start
```

## 3. What to Watch For

1.  **Initialization**:
    *   `✅ Components initialized`
    *   `💱 Checking for wSOL...` (It might wrap SOL if you don't have wSOL).

2.  **Scanning**:
    *   `📡 Listening for new Raydium pools...`

3.  **The Snipe**:
    *   Wait for a new pool (or trigger one if you can, otherwise just wait).
    *   **Log**: `🎯 SNIPE SIGNAL: ...`
    *   **Log**: `📤 Submitting bundle to Jito...`
    *   **Log**: `✅ Buy executed: <BundleID>`

4.  **The Position**:
    *   **Log**: `   └─ Confirmed on-chain balance: ...`
    *   **Log**: `   └─ Fetched real entry price: ...`
    *   **Log**: `🧠 AI Analysis...`

5.  **The Exit**:
    *   The bot should track the price.
    *   You can manually stop it (Ctrl+C) if it holds too long, or wait for TP/SL.
    *   **Emergency Sell**: If you need to exit immediately, you might need to use a separate tool (Phantom/Jupiter) since the bot doesn't have a manual CLI command yet, OR let the bot hit its Stop Loss.

## 4. Post-Test
- If successful, you can increase `SOL_WAGER_AMOUNT` gradually.
- If it fails, check the logs immediately.
