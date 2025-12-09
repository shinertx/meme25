import "dotenv/config";
import { Connection, Keypair } from "@solana/web3.js";
import { Janitor } from "../src/core/Janitor";
import bs58 from "bs58";

async function testJanitor() {
    console.log("🧪 Testing Janitor...");
    const connection = new Connection(process.env.RPC_URL || "https://api.mainnet-beta.solana.com");
    
    let keypair: Keypair;
    if (process.env.SOLANA_PRIVATE_KEY) {
        const secretKey = Uint8Array.from(JSON.parse(process.env.SOLANA_PRIVATE_KEY));
        keypair = Keypair.fromSecretKey(secretKey);
    } else if (process.env.PRIVATE_KEY) {
        keypair = Keypair.fromSecretKey(bs58.decode(process.env.PRIVATE_KEY));
    } else {
        throw new Error("SOLANA_PRIVATE_KEY or PRIVATE_KEY missing");
    }

    // Correct constructor: connection, keypair
    const janitor = new Janitor(connection, keypair);

    console.log("Running cleanup check...");
    
    // Correct method: cleanupEmptyTokenAccounts
    await janitor.cleanupEmptyTokenAccounts();
    console.log("✅ Janitor check complete.");
}

testJanitor().catch(console.error);
