"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
require("dotenv/config");
const web3_js_1 = require("@solana/web3.js");
const spl_token_1 = require("@solana/spl-token");
const JitoExecutor_1 = require("./services/JitoExecutor");
const SenderExecutor_1 = require("./services/SenderExecutor");
const BlockhashManager_1 = require("./services/BlockhashManager");
const SniperEngine_1 = require("./core/SniperEngine");
const MigrationListener_1 = require("./core/MigrationListener");
const SentientBrain_1 = require("./core/SentientBrain");
const Janitor_1 = require("./core/Janitor");
const config_1 = require("./config");
const bs58_1 = __importDefault(require("bs58"));
const Whitelist_1 = require("./services/Whitelist");
const PumpPreCog_1 = require("./core/PumpPreCog");
const Dashboard_1 = require("./core/Dashboard");
// dotenv.config(); // Removed as we use import "dotenv/config"
async function main() {
    console.log("🤖 Sentient Scavenger v1.0 - Initializing...");
    // 1. Load environment
    const privateKeyString = process.env.SOLANA_PRIVATE_KEY;
    if (!privateKeyString) {
        throw new Error("SOLANA_PRIVATE_KEY not set in .env");
    }
    const rpcUrl = process.env.SOLANA_RPC_URL;
    if (!rpcUrl) {
        throw new Error("SOLANA_RPC_URL not set in .env");
    }
    const openaiApiKey = process.env.OPENAI_API_KEY;
    if (!openaiApiKey) {
        throw new Error("OPENAI_API_KEY not set in .env");
    }
    // 2. Initialize keypair
    let keypair;
    try {
        if (privateKeyString.startsWith("[")) {
            const bytes = JSON.parse(privateKeyString);
            keypair = web3_js_1.Keypair.fromSecretKey(new Uint8Array(bytes));
        }
        else {
            keypair = web3_js_1.Keypair.fromSecretKey(bs58_1.default.decode(privateKeyString));
        }
    }
    catch (err) {
        throw new Error("Failed to parse PRIVATE_KEY: " + err);
    }
    console.log(`💰 Wallet: ${keypair.publicKey.toBase58()}`);
    // 3. Initialize connection
    const connection = new web3_js_1.Connection(rpcUrl, "processed");
    console.log(`🔗 Connected to: ${rpcUrl}`);
    // 4. Initialize components
    const jitoExecutor = new JitoExecutor_1.JitoExecutor(connection, keypair);
    const senderExecutor = new SenderExecutor_1.SenderExecutor(connection, keypair);
    const sniperEngine = new SniperEngine_1.SniperEngine(connection, keypair, jitoExecutor, senderExecutor);
    const sentientBrain = new SentientBrain_1.SentientBrain(connection, openaiApiKey, sniperEngine, keypair.publicKey);
    await sentientBrain.loadState();
    const whitelist = new Whitelist_1.Whitelist(config_1.WHITELIST_TTL_MS);
    const migrationListener = new MigrationListener_1.MigrationListener(connection, sniperEngine, sentientBrain, rpcUrl, whitelist);
    const pumpPreCog = new PumpPreCog_1.PumpPreCog(connection, rpcUrl, whitelist);
    const janitor = new Janitor_1.Janitor(connection, keypair);
    console.log("✅ Components initialized");
    let shuttingDown = false;
    const gracefulShutdown = async (reason) => {
        if (shuttingDown)
            return;
        shuttingDown = true;
        console.log(`\n🛑 Shutdown requested (${reason}). Closing positions...`);
        try {
            await sentientBrain.closeAll();
        }
        catch (err) {
            console.error("Error closing positions:", err);
        }
        finally {
            process.exit(0);
        }
    };
    process.on("SIGINT", () => gracefulShutdown("SIGINT"));
    process.on("SIGTERM", () => gracefulShutdown("SIGTERM"));
    process.on("unhandledRejection", (err) => {
        console.error("Unhandled rejection:", err);
        gracefulShutdown("unhandled rejection");
    });
    // 5. Pre-wrap SOL (Mandatory for Speed)
    console.log("💱 Checking for wSOL...");
    try {
        const wsolAta = await (0, spl_token_1.getAssociatedTokenAddress)(new web3_js_1.PublicKey(config_1.WSOL_MINT), keypair.publicKey);
        let currentBalance = 0;
        try {
            const bal = await connection.getTokenAccountBalance(wsolAta);
            currentBalance = bal.value.uiAmount || 0;
        }
        catch (e) {
            // Account doesn't exist
        }
        if (currentBalance < 0.01) {
            if (config_1.DRY_RUN) {
                console.log(`⚠️  Low wSOL balance (${currentBalance}), but DRY_RUN is active. Skipping wrap.`);
            }
            else {
                // Calculate safe wrap amount (leave 0.02 SOL for gas)
                const solBalance = await connection.getBalance(keypair.publicKey);
                const safeWrapAmount = Math.max(0, solBalance - 20000000); // Leave 0.02 SOL
                const targetWrap = 100000000; // Target 0.1 SOL total wSOL
                if (safeWrapAmount > 0) {
                    const amountToWrap = Math.min(safeWrapAmount, targetWrap);
                    console.log(`⚠️  Low wSOL balance (${currentBalance}), wrapping ${(amountToWrap / 1e9).toFixed(4)} SOL...`);
                    const { SystemProgram, Transaction, sendAndConfirmTransaction } = await Promise.resolve().then(() => __importStar(require("@solana/web3.js")));
                    const { createAssociatedTokenAccountIdempotentInstruction, createSyncNativeInstruction } = await Promise.resolve().then(() => __importStar(require("@solana/spl-token")));
                    const tx = new Transaction();
                    tx.add(createAssociatedTokenAccountIdempotentInstruction(keypair.publicKey, wsolAta, keypair.publicKey, new web3_js_1.PublicKey(config_1.WSOL_MINT)), SystemProgram.transfer({
                        fromPubkey: keypair.publicKey,
                        toPubkey: wsolAta,
                        lamports: amountToWrap
                    }), createSyncNativeInstruction(wsolAta));
                    await sendAndConfirmTransaction(connection, tx, [keypair]);
                    console.log(`✅ Wrapped ${(amountToWrap / 1e9).toFixed(4)} SOL successfully.`);
                }
                else {
                    console.warn("⚠️ Not enough SOL to wrap (need gas). Skipping.");
                }
            }
        }
        else {
            console.log(`✅ wSOL Balance: ${currentBalance} (Ready)`);
        }
    }
    catch (e) {
        console.error("❌ Failed to check/wrap wSOL:", e);
        process.exit(1); // Fail fast if we can't prepare
    }
    // 5. Start infrastructure
    console.log("🚀 Starting infrastructure...");
    await (0, BlockhashManager_1.initializeBlockhashManager)(connection);
    // 6. Start Pre-Cog producer
    await pumpPreCog.start();
    // 6. Start the Reflex (listener)
    console.log("👀 Starting The Reflex (listener)...");
    await migrationListener.startListening();
    // 7. Start the Janitor
    console.log("🧹 Starting The Janitor...");
    await janitor.startMaintenanceLoop(config_1.JANITOR_INTERVAL);
    // 8. Start Dashboard
    const dashboard = new Dashboard_1.Dashboard(3333, whitelist, sentientBrain, migrationListener, pumpPreCog);
    dashboard.start();
    // 9. Heartbeat / Monitoring
    setInterval(() => {
        const positions = sentientBrain.getActivePositions().length;
        const wlSize = whitelist.size();
        const now = Date.now();
        const sinceLog = migrationListener.lastLogAt
            ? Math.round((now - migrationListener.lastLogAt) / 1000)
            : -1;
        console.log(`❤️ Heartbeat | Active Positions: ${positions} | Whitelist: ${wlSize} entries | Last Raydium Log: ${sinceLog}s ago`);
        if (sinceLog > 180) {
            console.warn("⚠️ No Raydium logs seen in >180s. Check RPC/WSS connectivity.");
        }
    }, 60000);
    console.log("✅ All systems online. Awaiting migrations...\n");
    console.log("═══════════════════════════════════════════════════════════");
    console.log("🎯 MemeSnipe Scavenger Ready");
    console.log("   The Reflex: Active");
    console.log("   The Brain: Active");
    console.log("   The Janitor: Active");
    console.log("═══════════════════════════════════════════════════════════\n");
    // Keep the process alive
    await new Promise(() => { });
}
main().catch((err) => {
    console.error("Fatal error:", err);
    process.exit(1);
});
//# sourceMappingURL=main.js.map