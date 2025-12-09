// ==========================================
// TRADING CONFIG
// ==========================================
export const SOL_WAGER_AMOUNT = 0.001; // Risk per trade (in SOL)
export const JITO_TIP_CAP = 0.002; // Max tip to Jito (in SOL)
export const SLIPPAGE_BPS = 1500; // 15% - extreme volatility

// ==========================================
// PUMP.FUN & RAYDIUM CONSTANTS
// ==========================================
export const PUMP_MIGRATION_AUTH = "39azUYFWPz3VHgKCf3VChUwbpURdCHRxjWVowf5jUJjg";
export const RAYDIUM_V4_PROGRAM = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
export const RAYDIUM_AUTHORITY = "5Q544fKrFoe6tsEbD7K5DRgT5K6ffDS1G5DvGudLh61";
export const RAYDIUM_PROGRAM_ID = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
export const SYSTEM_PROGRAM = "11111111111111111111111111111111";
export const TOKEN_PROGRAM = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

// ==========================================
// TOKEN & MINT
// ==========================================
export const WSOL_MINT = "So11111111111111111111111111111111111111112";
export const WSOL_DECIMALS = 9;
export const DEFAULT_TOKEN_DECIMALS = 6;

// ==========================================
// AI SCORING THRESHOLDS
// ==========================================
export const AI_SCORE_IMMEDIATE_SELL = 5;
export const AI_SCORE_HOLD_LONG = 8;
export const TAKE_PROFIT_LOW = 0.5; // 50% for score 5-8
export const TAKE_PROFIT_HIGH = 2.0; // 200% for score > 8
export const STOP_LOSS_LOW = -0.1; // -10% for score 5-8
export const STOP_LOSS_HIGH = -0.15; // -15% for score > 8

// ==========================================
// POLLING INTERVALS (milliseconds)
// ==========================================
export const BLOCKHASH_POLL_INTERVAL = 500; // Reduced to 500ms (Paid Plan)
export const PRICE_POLL_INTERVAL = 1000; // Reduced to 1s (Paid Plan)
export const JANITOR_INTERVAL = 60000; // 60 seconds
export const BUNDLE_CONFIRMATION_POLL = 500; // 500ms
export const BUNDLE_CONFIRMATION_TIMEOUT = 30000; // 30 seconds

// ==========================================
// JITO TIP ACCOUNTS
// ==========================================
export const JITO_TIP_ACCOUNTS = [
  "96gYZGLnJYVFmbjzopPSU6QiEV5nWosPt9G5zJY6FwqF",
  "HFqU5x63VTqvQss8hp11i4wVV3cWJCMvm8L2M685pump",
  "Cw8CFyM9FkoMi623serSgV7L3XwwLsNaWilt1S4pump",
  "ADuUkR4mksrfDA9E2YYAso44NjAReVRp7V4phpFdrVgH",
  "DttWaC3NeTUJiFDrzMjc3JazayKDTZpr5VK9x5XCSu5H",
  "3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnIzE60d8",
  "DfXygSm4jCyNCybVYYK6DwvWqjKkf8tVgZ5fAw2ere1",
  "ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49",
  // Helius Sender Tip Accounts
  "4ACfpUFoaSD9bfPdeu6DBt89gB6ENTeHBXCAi87NhDEE",
  "D2L6yPZ2FmmmTKPgzaMKdhu6EWZcTpLy1Vhx8uvZe7NZ",
  "9bnz4RShgq1hAnLnZbP8kbgBg1kEmcJBYQq3gQbmnSta",
  "5VY91ws6B2hMmBFRsXkoAAdsPHBJwRfBht4DXox3xkwn",
  "2nyhqdwKcJZR2vcqCyrYsaPVdAnFoJjiksCXJ7hfEYgD",
  "2q5pghRs6arqVjRvT5gfgWfWcHWmw1ZuCzphgd5KfWGJ",
  "wyvPkWjVZz1M8fHQnMMCDTQDbkManefNNhweYk5WkcF",
  "3KCKozbAaF75qEU33jtzozcJ29yJuaLJTy2jFdzUY8bT",
  "4vieeGHPYPG2MmyPRcYjdiDmmhN3ww7hsFNap8pVN3Ey",
  "4TQLFNWK8AovT1gFvda5jfw2oJeRMKEmw7aH6MGBJ3or"
];

export const JITO_BLOCK_ENGINE_URL = process.env.JITO_BLOCK_ENGINE_URL || "https://mainnet.block-engine.jito.wtf/api/v1/bundles";

// ==========================================
// MODE & LOGGING
// ==========================================
export const DRY_RUN = (process.env.DRY_RUN === "true");
export const LOG_LEVEL = process.env.LOG_LEVEL || "info";
export const RELAX_FILTERS =
  process.env.RELAX_FILTERS === "true" || process.env.RELAX_FILTERS === "1";

// ==========================================
// PRE-COG / SAFETY
// ==========================================
export const PUMP_PROGRAM = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
export const MAX_CABAL_PCT = 20;
export const WHITELIST_TTL_MS = 10 * 60 * 1000;
export const MOONBAG_THRESHOLD_SOL = 6.0; // switch to moonbag when est. position value exceeds this
export const MOONBAG_SELL_PCT = 0.8; // sell 80%, hold 20%
export const MIN_CURVE_PREFILTER = Number(process.env.MIN_CURVE_PREFILTER || 95); // curve % threshold for precog
