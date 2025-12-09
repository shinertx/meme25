import "dotenv/config";
import { Connection, Keypair } from "@solana/web3.js";
import { SentientBrain } from "../src/core/SentientBrain";
import { SniperEngine } from "../src/core/SniperEngine";
import { JitoExecutor } from "../src/services/JitoExecutor";
import { initializeBlockhashManager } from "../src/services/BlockhashManager";
import bs58 from "bs58";

async function testOpenAI() {
    console.log("🧪 Testing OpenAI Integration...");

    // 1. Setup Dependencies
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

    await initializeBlockhashManager(connection);
    const jitoExecutor = new JitoExecutor(connection, keypair);
    const sniperEngine = new SniperEngine(connection, keypair, jitoExecutor);

    // 2. Initialize SentientBrain
    const openaiApiKey = process.env.OPENAI_API_KEY;
    if (!openaiApiKey) {
        throw new Error("OPENAI_API_KEY missing in .env");
    }

    const brain = new SentientBrain(connection, openaiApiKey, sniperEngine, keypair.publicKey);

    // 3. Mock Token Data
    const mockTokenData = {
        name: "Test Meme Coin",
        symbol: "TEST",
        description: "A coin for testing the AI analysis module. It has high meme potential because it is a test.",
        twitter: "https://twitter.com/testmemecoin"
    };

    console.log("Sending request to OpenAI...");
    const startTime = Date.now();

    try {
        const score = await brain.analyzeToken("MockMintAddress11111111111111111111111111", mockTokenData);
        const duration = Date.now() - startTime;

        console.log(`✅ OpenAI Response Received in ${duration}ms`);
        console.log(`   Score: ${score}/10`);

        if (score >= 1 && score <= 10) {
            console.log("✅ Score is within valid range (1-10).");
            process.exit(0);
        } else {
            console.error("❌ Score is OUT of range.");
            process.exit(1);
        }

    } catch (error) {
        console.error("❌ OpenAI Test Failed:", error);
        process.exit(1);
    }
}

testOpenAI().catch(console.error);
