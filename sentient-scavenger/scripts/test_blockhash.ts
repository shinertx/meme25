
import "dotenv/config";
import { Connection } from "@solana/web3.js";
import { initializeBlockhashManager, getCachedBlockhash } from "../src/services/BlockhashManager";

async function testBlockhash() {
    console.log("🧪 Testing BlockhashManager...");
    const connection = new Connection(process.env.SOLANA_RPC_URL || "");
    
    await initializeBlockhashManager(connection);
    
    const initial = getCachedBlockhash();
    console.log("Initial Blockhash:", initial?.blockhash);

    console.log("Waiting 2 seconds for update...");
    await new Promise(r => setTimeout(r, 2000));

    const updated = getCachedBlockhash();
    console.log("Updated Blockhash:", updated?.blockhash);

    if (initial?.blockhash !== updated?.blockhash) {
        console.log("✅ Blockhash updated successfully.");
    } else {
        console.warn("⚠️ Blockhash did not update (might be slow network or same block).");
    }
    process.exit(0);
}

testBlockhash().catch(console.error);
