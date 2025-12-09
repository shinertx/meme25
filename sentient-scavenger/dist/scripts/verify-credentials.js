"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
const dotenv_1 = __importDefault(require("dotenv"));
const web3_js_1 = require("@solana/web3.js");
const openai_1 = __importDefault(require("openai"));
// @ts-ignore
const bs58_1 = __importDefault(require("bs58"));
const axios_1 = __importDefault(require("axios"));
// Load env
dotenv_1.default.config();
const COLORS = {
    GREEN: "\x1b[32m",
    RED: "\x1b[31m",
    RESET: "\x1b[0m",
    YELLOW: "\x1b[33m",
};
async function verify() {
    console.log(`\n🕵️  ${COLORS.YELLOW}STARTING CREDENTIAL AUDIT...${COLORS.RESET}\n`);
    let allPassed = true;
    // 1. VERIFY SOLANA RPC (HELIUS)
    try {
        const rpcUrl = process.env.SOLANA_RPC_URL;
        if (!rpcUrl)
            throw new Error("SOLANA_RPC_URL missing in .env");
        process.stdout.write(`📡 Testing RPC Connection (${rpcUrl.slice(0, 25)}...)... `);
        const connection = new web3_js_1.Connection(rpcUrl);
        const version = await connection.getVersion();
        const slot = await connection.getSlot();
        console.log(`${COLORS.GREEN}✅ PASS (v${version["solana-core"]}, Slot: ${slot})${COLORS.RESET}`);
    }
    catch (e) {
        console.log(`${COLORS.RED}❌ FAIL: ${e.message}${COLORS.RESET}`);
        allPassed = false;
    }
    // 2. VERIFY WALLET PRIVATE KEY
    try {
        process.stdout.write(`🔑 Testing Wallet Private Key... `);
        const pkFormat = process.env.SOLANA_PRIVATE_KEY;
        if (!pkFormat)
            throw new Error("SOLANA_PRIVATE_KEY missing in .env");
        let keypair;
        if (pkFormat.includes("[")) {
            const raw = Uint8Array.from(JSON.parse(pkFormat));
            keypair = web3_js_1.Keypair.fromSecretKey(raw);
        }
        else {
            keypair = web3_js_1.Keypair.fromSecretKey(bs58_1.default.decode(pkFormat));
        }
        console.log(`${COLORS.GREEN}✅ PASS (Pubkey: ${keypair.publicKey.toBase58()})${COLORS.RESET}`);
        // Check Balance
        const connection = new web3_js_1.Connection(process.env.SOLANA_RPC_URL);
        const balance = await connection.getBalance(keypair.publicKey);
        console.log(`   💰 Balance: ${COLORS.YELLOW}${balance / 1e9} SOL${COLORS.RESET}`);
        if (balance < 0.05 * 1e9) {
            console.log(`   ⚠️  ${COLORS.RED}WARNING: Low Balance (< 0.05 SOL)${COLORS.RESET}`);
        }
    }
    catch (e) {
        console.log(`${COLORS.RED}❌ FAIL: ${e.message}${COLORS.RESET}`);
        allPassed = false;
    }
    // 3. VERIFY OPENAI API
    try {
        process.stdout.write(`🧠 Testing OpenAI API... `);
        const apiKey = process.env.OPENAI_API_KEY;
        if (!apiKey)
            throw new Error("OPENAI_API_KEY missing in .env");
        const openai = new openai_1.default({ apiKey });
        const models = await openai.models.list();
        const gpt4 = models.data.find(m => m.id.includes("gpt-4"));
        if (gpt4) {
            console.log(`${COLORS.GREEN}✅ PASS (Found ${gpt4.id})${COLORS.RESET}`);
        }
        else {
            console.log(`${COLORS.YELLOW}⚠️  PASS (Connected, but GPT-4 not found in list)${COLORS.RESET}`);
        }
    }
    catch (e) {
        console.log(`${COLORS.RED}❌ FAIL: ${e.message}${COLORS.RESET}`);
        allPassed = false;
    }
    // 4. VERIFY JITO BLOCK ENGINE
    try {
        process.stdout.write(`⚡ Testing Jito Block Engine Connectivity... `);
        // Simple HTTP ping to check if endpoint is reachable
        const jitoUrl = process.env.JITO_BLOCK_ENGINE_URL || "https://mainnet.block-engine.jito.wtf/api/v1/bundles";
        // Note: Jito doesn't have a simple GET health check on the bundle endpoint, 
        // but we can check if the DNS resolves and we get a 405/404 (meaning server is there)
        // instead of Connection Refused.
        await axios_1.default.get(jitoUrl).catch(e => {
            if (e.response) {
                // If we get a response (even 404/405), the server is reachable
                console.log(`${COLORS.GREEN}✅ PASS (Server Reachable)${COLORS.RESET}`);
            }
            else {
                throw e;
            }
        });
    }
    catch (e) {
        console.log(`${COLORS.RED}❌ FAIL: ${e.message}${COLORS.RESET}`);
        // Jito is critical, but sometimes blocks simple GETs. 
        // We mark as fail but it might just be the test method.
        allPassed = false;
    }
    console.log("\n" + "=".repeat(40));
    if (allPassed) {
        console.log(`${COLORS.GREEN}🚀 SYSTEM READY FOR DEPLOYMENT${COLORS.RESET}`);
        process.exit(0);
    }
    else {
        console.log(`${COLORS.RED}🛑 SYSTEM CHECKS FAILED - DO NOT DEPLOY${COLORS.RESET}`);
        process.exit(1);
    }
}
verify();
//# sourceMappingURL=verify-credentials.js.map