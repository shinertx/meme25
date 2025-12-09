import "dotenv/config";
import { Connection, Keypair, SystemProgram, Transaction } from "@solana/web3.js";
import { JitoExecutor } from "../src/services/JitoExecutor";
import { initializeBlockhashManager } from "../src/services/BlockhashManager";
import bs58 from "bs58";

async function testJito() {
    console.log("🧪 Testing Jito Executor...");

    // 1. Setup Connection & Wallet
    const connection = new Connection(process.env.RPC_URL || "https://api.mainnet-beta.solana.com");
    
    let keypair: Keypair;
    if (process.env.SOLANA_PRIVATE_KEY) {
        const secretKey = Uint8Array.from(JSON.parse(process.env.SOLANA_PRIVATE_KEY));
        keypair = Keypair.fromSecretKey(secretKey);
    } else if (process.env.PRIVATE_KEY) {
        keypair = Keypair.fromSecretKey(bs58.decode(process.env.PRIVATE_KEY));
    } else {
        throw new Error("PRIVATE_KEY missing");
    }

    // 2. Initialize Blockhash Manager (needed for JitoExecutor)
    console.log("Initializing Blockhash Manager...");
    await initializeBlockhashManager(connection);

    // 3. Initialize Jito Executor
    const jitoExecutor = new JitoExecutor(connection, keypair);

    // 4. Create a dummy transaction (Self-transfer 0 SOL)
    console.log("Creating dummy transaction...");
    const tx = new Transaction().add(
        SystemProgram.transfer({
            fromPubkey: keypair.publicKey,
            toPubkey: keypair.publicKey,
            lamports: 0,
        })
    );

    // 5. Execute via Jito (Dry Run)
    console.log("Executing Jito Bundle (Dry Run)...");
    try {
        const bundleId = await jitoExecutor.executeAndConfirm(tx, true);
        
        if (bundleId && bundleId.startsWith("DRY_RUN")) {
            console.log(`✅ Jito Test Passed! Mock Bundle ID: ${bundleId}`);
            process.exit(0);
        } else if (bundleId) {
            console.log(`⚠️ Jito Test returned real Bundle ID (Check if DRY_RUN is off): ${bundleId}`);
        } else {
            console.error("❌ Jito Test Failed: No Bundle ID returned.");
            process.exit(1);
        }

    } catch (error) {
        console.error("❌ Jito Test Failed with error:", error);
        process.exit(1);
    }
}

testJito().catch(console.error);
