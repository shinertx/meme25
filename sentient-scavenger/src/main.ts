import "dotenv/config";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { getAssociatedTokenAddress } from "@solana/spl-token";
import { JitoExecutor } from "./services/JitoExecutor";
import { SenderExecutor } from "./services/SenderExecutor";
import { initializeBlockhashManager } from "./services/BlockhashManager";
import { SniperEngine } from "./core/SniperEngine";
import { MigrationListener } from "./core/MigrationListener";
import { SentientBrain } from "./core/SentientBrain";
import { Janitor } from "./core/Janitor";
import { JANITOR_INTERVAL, WSOL_MINT, DRY_RUN, WHITELIST_TTL_MS } from "./config";
import bs58 from "bs58";
import { Whitelist } from "./services/Whitelist";
import { PumpPreCog } from "./core/PumpPreCog";
import { Dashboard } from "./core/Dashboard";

// dotenv.config(); // Removed as we use import "dotenv/config"

async function main(): Promise<void> {
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
  let keypair: Keypair;
  try {
    if (privateKeyString.startsWith("[")) {
      const bytes = JSON.parse(privateKeyString);
      keypair = Keypair.fromSecretKey(new Uint8Array(bytes));
    } else {
      keypair = Keypair.fromSecretKey(bs58.decode(privateKeyString));
    }
  } catch (err) {
    throw new Error("Failed to parse PRIVATE_KEY: " + err);
  }

  console.log(`💰 Wallet: ${keypair.publicKey.toBase58()}`);

  // 3. Initialize connection
  const connection = new Connection(rpcUrl, "processed");
  console.log(`🔗 Connected to: ${rpcUrl}`);

  // 4. Initialize components
  const jitoExecutor = new JitoExecutor(connection, keypair);
  const senderExecutor = new SenderExecutor(connection, keypair);
  const sniperEngine = new SniperEngine(connection, keypair, jitoExecutor, senderExecutor);
  const sentientBrain = new SentientBrain(connection, openaiApiKey, sniperEngine, keypair.publicKey);
  await sentientBrain.loadState();
  const whitelist = new Whitelist(WHITELIST_TTL_MS);
  const migrationListener = new MigrationListener(
    connection,
    sniperEngine,
    sentientBrain,
    rpcUrl,
    whitelist
  );
  const pumpPreCog = new PumpPreCog(connection, rpcUrl, whitelist);
  const janitor = new Janitor(connection, keypair);

  console.log("✅ Components initialized");

  let shuttingDown = false;
  const gracefulShutdown = async (reason: string) => {
    if (shuttingDown) return;
    shuttingDown = true;
    console.log(`\n🛑 Shutdown requested (${reason}). Closing positions...`);
    try {
      await sentientBrain.closeAll();
    } catch (err) {
      console.error("Error closing positions:", err);
    } finally {
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
    const wsolAta = await getAssociatedTokenAddress(
      new PublicKey(WSOL_MINT),
      keypair.publicKey
    );
    
    let currentBalance = 0;
    try {
      const bal = await connection.getTokenAccountBalance(wsolAta);
      currentBalance = bal.value.uiAmount || 0;
    } catch (e) {
      // Account doesn't exist
    }

    if (currentBalance < 0.01) {
      if (DRY_RUN) {
        console.log(`⚠️  Low wSOL balance (${currentBalance}), but DRY_RUN is active. Skipping wrap.`);
      } else {
        // Calculate safe wrap amount (leave 0.02 SOL for gas)
        const solBalance = await connection.getBalance(keypair.publicKey);
        const safeWrapAmount = Math.max(0, solBalance - 20_000_000); // Leave 0.02 SOL
        const targetWrap = 100_000_000; // Target 0.1 SOL total wSOL

        if (safeWrapAmount > 0) {
           const amountToWrap = Math.min(safeWrapAmount, targetWrap);
           console.log(`⚠️  Low wSOL balance (${currentBalance}), wrapping ${(amountToWrap / 1e9).toFixed(4)} SOL...`);

           const { SystemProgram, Transaction, sendAndConfirmTransaction } = await import("@solana/web3.js");
           const { createAssociatedTokenAccountIdempotentInstruction, createSyncNativeInstruction } = await import("@solana/spl-token");

           const tx = new Transaction();
           tx.add(
             createAssociatedTokenAccountIdempotentInstruction(
               keypair.publicKey,
               wsolAta,
               keypair.publicKey,
               new PublicKey(WSOL_MINT)
             ),
             SystemProgram.transfer({
               fromPubkey: keypair.publicKey,
               toPubkey: wsolAta,
               lamports: amountToWrap
             }),
             createSyncNativeInstruction(wsolAta)
           );
           
           await sendAndConfirmTransaction(connection, tx, [keypair]);
           console.log(`✅ Wrapped ${(amountToWrap / 1e9).toFixed(4)} SOL successfully.`);
        } else {
           console.warn("⚠️ Not enough SOL to wrap (need gas). Skipping.");
        }
      }
    } else {
      console.log(`✅ wSOL Balance: ${currentBalance} (Ready)`);
    }
  } catch (e) {
    console.error("❌ Failed to check/wrap wSOL:", e);
    process.exit(1); // Fail fast if we can't prepare
  }

  // 5. Start infrastructure
  console.log("🚀 Starting infrastructure...");
  await initializeBlockhashManager(connection);

  // 6. Start Pre-Cog producer
  await pumpPreCog.start();

  // 6. Start the Reflex (listener)
  console.log("👀 Starting The Reflex (listener)...");
  await migrationListener.startListening();

  // 7. Start the Janitor
  console.log("🧹 Starting The Janitor...");
  await janitor.startMaintenanceLoop(JANITOR_INTERVAL);

  // 8. Start Dashboard
  const dashboard = new Dashboard(3333, whitelist, sentientBrain, migrationListener, pumpPreCog);
  dashboard.start();

  // 9. Heartbeat / Monitoring
  setInterval(() => {
    const positions = sentientBrain.getActivePositions().length;
    const wlSize = whitelist.size();
    const now = Date.now();
    const sinceLog = migrationListener.lastLogAt
      ? Math.round((now - migrationListener.lastLogAt) / 1000)
      : -1;
    console.log(
      `❤️ Heartbeat | Active Positions: ${positions} | Whitelist: ${wlSize} entries | Last Raydium Log: ${sinceLog}s ago`
    );
    if (sinceLog > 180) {
      console.warn("⚠️ No Raydium logs seen in >180s. Check RPC/WSS connectivity.");
    }
  }, 60_000);

  console.log("✅ All systems online. Awaiting migrations...\n");
  console.log("═══════════════════════════════════════════════════════════");
  console.log("🎯 MemeSnipe Scavenger Ready");
  console.log("   The Reflex: Active");
  console.log("   The Brain: Active");
  console.log("   The Janitor: Active");
  console.log("═══════════════════════════════════════════════════════════\n");

  // Keep the process alive
  await new Promise(() => {});
}

main().catch((err) => {
  console.error("Fatal error:", err);
  process.exit(1);
});
