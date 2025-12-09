"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.MIN_CURVE_PREFILTER = exports.MOONBAG_SELL_PCT = exports.MOONBAG_THRESHOLD_SOL = exports.WHITELIST_TTL_MS = exports.MAX_CABAL_PCT = exports.PUMP_PROGRAM = exports.RELAX_FILTERS = exports.LOG_LEVEL = exports.DRY_RUN = exports.JITO_BLOCK_ENGINE_URL = exports.JITO_TIP_ACCOUNTS = exports.BUNDLE_CONFIRMATION_TIMEOUT = exports.BUNDLE_CONFIRMATION_POLL = exports.JANITOR_INTERVAL = exports.PRICE_POLL_INTERVAL = exports.BLOCKHASH_POLL_INTERVAL = exports.STOP_LOSS_HIGH = exports.STOP_LOSS_LOW = exports.TAKE_PROFIT_HIGH = exports.TAKE_PROFIT_LOW = exports.AI_SCORE_HOLD_LONG = exports.AI_SCORE_IMMEDIATE_SELL = exports.DEFAULT_TOKEN_DECIMALS = exports.WSOL_DECIMALS = exports.WSOL_MINT = exports.TOKEN_PROGRAM = exports.SYSTEM_PROGRAM = exports.RAYDIUM_PROGRAM_ID = exports.RAYDIUM_AUTHORITY = exports.RAYDIUM_V4_PROGRAM = exports.PUMP_MIGRATION_AUTH = exports.SLIPPAGE_BPS = exports.JITO_TIP_CAP = exports.SOL_WAGER_AMOUNT = void 0;
// ==========================================
// TRADING CONFIG
// ==========================================
exports.SOL_WAGER_AMOUNT = 0.001; // Risk per trade (in SOL)
exports.JITO_TIP_CAP = 0.002; // Max tip to Jito (in SOL)
exports.SLIPPAGE_BPS = 1500; // 15% - extreme volatility
// ==========================================
// PUMP.FUN & RAYDIUM CONSTANTS
// ==========================================
exports.PUMP_MIGRATION_AUTH = "39azUYFWPz3VHgKCf3VChUwbpURdCHRxjWVowf5jUJjg";
exports.RAYDIUM_V4_PROGRAM = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
exports.RAYDIUM_AUTHORITY = "5Q544fKrFoe6tsEbD7K5DRgT5K6ffDS1G5DvGudLh61";
exports.RAYDIUM_PROGRAM_ID = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
exports.SYSTEM_PROGRAM = "11111111111111111111111111111111";
exports.TOKEN_PROGRAM = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
// ==========================================
// TOKEN & MINT
// ==========================================
exports.WSOL_MINT = "So11111111111111111111111111111111111111112";
exports.WSOL_DECIMALS = 9;
exports.DEFAULT_TOKEN_DECIMALS = 6;
// ==========================================
// AI SCORING THRESHOLDS
// ==========================================
exports.AI_SCORE_IMMEDIATE_SELL = 5;
exports.AI_SCORE_HOLD_LONG = 8;
exports.TAKE_PROFIT_LOW = 0.5; // 50% for score 5-8
exports.TAKE_PROFIT_HIGH = 2.0; // 200% for score > 8
exports.STOP_LOSS_LOW = -0.1; // -10% for score 5-8
exports.STOP_LOSS_HIGH = -0.15; // -15% for score > 8
// ==========================================
// POLLING INTERVALS (milliseconds)
// ==========================================
exports.BLOCKHASH_POLL_INTERVAL = 500; // Reduced to 500ms (Paid Plan)
exports.PRICE_POLL_INTERVAL = 1000; // Reduced to 1s (Paid Plan)
exports.JANITOR_INTERVAL = 60000; // 60 seconds
exports.BUNDLE_CONFIRMATION_POLL = 500; // 500ms
exports.BUNDLE_CONFIRMATION_TIMEOUT = 30000; // 30 seconds
// ==========================================
// JITO TIP ACCOUNTS
// ==========================================
exports.JITO_TIP_ACCOUNTS = [
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
exports.JITO_BLOCK_ENGINE_URL = process.env.JITO_BLOCK_ENGINE_URL || "https://mainnet.block-engine.jito.wtf/api/v1/bundles";
// ==========================================
// MODE & LOGGING
// ==========================================
exports.DRY_RUN = (process.env.DRY_RUN === "true");
exports.LOG_LEVEL = process.env.LOG_LEVEL || "info";
exports.RELAX_FILTERS = process.env.RELAX_FILTERS === "true" || process.env.RELAX_FILTERS === "1";
// ==========================================
// PRE-COG / SAFETY
// ==========================================
exports.PUMP_PROGRAM = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
exports.MAX_CABAL_PCT = 20;
exports.WHITELIST_TTL_MS = 10 * 60 * 1000;
exports.MOONBAG_THRESHOLD_SOL = 6.0; // switch to moonbag when est. position value exceeds this
exports.MOONBAG_SELL_PCT = 0.8; // sell 80%, hold 20%
exports.MIN_CURVE_PREFILTER = Number(process.env.MIN_CURVE_PREFILTER || 95); // curve % threshold for precog
//# sourceMappingURL=config.js.map