"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.MigrationListener = void 0;
const web3_js_1 = require("@solana/web3.js");
const config_1 = require("../config");
const axios_1 = __importDefault(require("axios"));
class MigrationListener {
    constructor(connection, sniperEngine, sentientBrain, heliusRpcUrl, whitelist) {
        this.queue = [];
        this.isProcessing = false;
        this.lastLogAt = Date.now();
        this.logCount = 0;
        this.connection = connection;
        this.sniperEngine = sniperEngine;
        this.sentientBrain = sentientBrain;
        this.heliusRpcUrl = heliusRpcUrl;
        this.whitelist = whitelist;
    }
    parsePoolKeysFromTx(keys) {
        // Raydium initialize2 account order assumption:
        // 3: id/pool, 4: authority, 5: openOrders, 6: lpMint, 7: baseMint, 8: quoteMint,
        // 9: baseVault, 10: quoteVault, 11: targetOrders, 14: marketProgramId, 15: marketId
        if (keys.length < 16)
            return null;
        try {
            const id = new web3_js_1.PublicKey(keys[3]);
            const authority = new web3_js_1.PublicKey(keys[4]);
            const openOrders = new web3_js_1.PublicKey(keys[5]);
            const lpMint = new web3_js_1.PublicKey(keys[6]);
            const baseMint = new web3_js_1.PublicKey(keys[7]);
            const quoteMint = new web3_js_1.PublicKey(keys[8]);
            const baseVault = new web3_js_1.PublicKey(keys[9]);
            const quoteVault = new web3_js_1.PublicKey(keys[10]);
            const targetOrders = new web3_js_1.PublicKey(keys[11]);
            const marketProgramId = new web3_js_1.PublicKey(keys[14]);
            const marketId = new web3_js_1.PublicKey(keys[15]);
            return {
                id,
                baseMint,
                quoteMint,
                lpMint,
                baseDecimals: 9,
                quoteDecimals: 9,
                lpDecimals: 9,
                version: 4,
                programId: new web3_js_1.PublicKey(config_1.RAYDIUM_V4_PROGRAM),
                authority,
                openOrders,
                targetOrders,
                baseVault,
                quoteVault,
                withdrawQueue: web3_js_1.PublicKey.default,
                lpVault: web3_js_1.PublicKey.default,
                marketVersion: 3,
                marketProgramId,
                marketId,
                marketAuthority: web3_js_1.PublicKey.default,
                marketBaseVault: web3_js_1.PublicKey.default,
                marketQuoteVault: web3_js_1.PublicKey.default,
                marketBids: web3_js_1.PublicKey.default,
                marketAsks: web3_js_1.PublicKey.default,
                marketEventQueue: web3_js_1.PublicKey.default,
                lookupTableAccount: web3_js_1.PublicKey.default,
            };
        }
        catch (e) {
            return null;
        }
    }
    extractMintFromLogs(logsStr) {
        const parts = logsStr.split(/\s+/);
        const candidate = parts.find((p) => p.length >= 32 && p.length <= 44 && /^[1-9A-HJ-NP-Za-km-z]+$/.test(p));
        return candidate || null;
    }
    getVelocity() {
        return this.logCount;
    }
    async startListening() {
        console.log(`👀 MigrationListener: Monitoring Raydium V4 (${config_1.RAYDIUM_V4_PROGRAM})`);
        // Subscribe to logs
        try {
            const programId = new web3_js_1.PublicKey(config_1.RAYDIUM_V4_PROGRAM);
            this.connection.onLogs(programId, async (logs, context) => {
                this.lastLogAt = Date.now();
                this.logCount++;
                const logs_string = logs.logs.join(" ").toLowerCase();
                // Show velocity every 1000 events to prove speed
                if (this.logCount % 1000 === 0) {
                    console.log(`⚡ Stream Velocity: Processed ${this.logCount} Raydium events. Current Slot: ${context.slot}`);
                    // Print a sample log to verify we can read the text
                    // console.log(`   Sample Log: ${logs.logs[0]?.substring(0, 100)}...`);
                }
                // Look for initialize2 instruction
                if (logs_string.includes("initialize2")) { // Reverted to strict 'initialize2' to reduce noise
                    console.log(`📡 RAW DATA: initialize2 detected in slot ${context.slot} (Signature: ${logs.signature.substring(0, 8)}...)`);
                    // If whitelist provided, try to extract mint from log to avoid needless RPC
                    if (this.whitelist) {
                        // Inline extraction to avoid TS issues
                        const parts = logs_string.split(/\s+/);
                        const mintFromLog = parts.find((p) => p.length >= 32 && p.length <= 44 && /^[1-9A-HJ-NP-Za-km-z]+$/.test(p)) || null;
                        if (mintFromLog) {
                            // Early skip if not whitelisted; actual consume happens in processMigrationLog
                            if (!this.whitelist.has(mintFromLog)) {
                                return;
                            }
                        }
                    }
                    // Push to queue to avoid 429s
                    this.queue.push({ signature: logs.signature, slot: context.slot });
                    this.processQueue();
                }
            }, "processed");
        }
        catch (err) {
            console.error("Failed to start listener:", err);
        }
        console.log(`✅ Listener active`);
    }
    async processQueue() {
        if (this.isProcessing)
            return;
        this.isProcessing = true;
        while (this.queue.length > 0) {
            const { signature, slot } = this.queue.shift();
            try {
                await this.processMigrationLog(signature, slot);
            }
            catch (err) {
                console.error("Error processing log:", err);
            }
            // Add a delay to be nice to the RPC (Rate Limit Protection)
            // Paid Plan: Reduced to 0ms (Firehose Mode)
            // await new Promise(resolve => setTimeout(resolve, 2000)); 
        }
        this.isProcessing = false;
    }
    // Filter 3: Metadata (Socials)
    async checkMetadata(mint) {
        try {
            const response = await axios_1.default.post(this.heliusRpcUrl, {
                jsonrpc: "2.0",
                id: "my-id",
                method: "getAsset",
                params: { id: mint }
            });
            const asset = response.data.result;
            // Check for socials in typical locations (Helius DAS format)
            const hasTwitter = Boolean(asset?.extensions?.twitter ||
                asset?.content?.metadata?.social?.twitter);
            const hasTelegram = Boolean(asset?.extensions?.telegram ||
                asset?.content?.metadata?.social?.telegram);
            const safe = hasTwitter || hasTelegram;
            console.log(`📝 Metadata: ${asset?.content?.metadata?.name} ($${asset?.content?.metadata?.symbol}) - Safe: ${safe}`);
            return {
                safe,
                data: {
                    name: asset?.content?.metadata?.name || "Unknown",
                    symbol: asset?.content?.metadata?.symbol || "???",
                    description: asset?.content?.metadata?.description || "",
                    twitter: asset?.extensions?.twitter
                }
            };
        }
        catch (e) {
            console.error("Metadata check failed:", e);
            return {
                safe: true,
                data: { name: "Unknown", symbol: "???", description: "" },
            };
        }
    }
    // Filter 4: The Cabal Filter (Insider Holdings)
    async checkCabal(mint) {
        try {
            const mintPk = new web3_js_1.PublicKey(mint);
            const [largestAccounts, supplyInfo] = await Promise.all([
                this.connection.getTokenLargestAccounts(mintPk),
                this.connection.getTokenSupply(mintPk),
            ]);
            if (!largestAccounts.value || largestAccounts.value.length === 0)
                return true;
            const supplyRaw = BigInt(supplyInfo.value.amount || "0");
            if (supplyRaw === BigInt(0)) {
                console.warn("⚠️ Unable to determine supply, skipping cabal check.");
                return true;
            }
            const sorted = largestAccounts.value
                .map((acc) => ({
                address: acc.address,
                amount: BigInt(acc.amount),
            }))
                .sort((a, b) => Number(b.amount - a.amount));
            let insiderSum = BigInt(0);
            const MAX_TRACKED = 10;
            for (let i = 0, counted = 0; i < sorted.length && counted < MAX_TRACKED; i++) {
                const position = sorted[i];
                const pct = (position.amount * BigInt(10000)) / supplyRaw; // basis points
                // Skip likely pool/curve accounts (>30%)
                if (pct > BigInt(3000))
                    continue;
                insiderSum += position.amount;
                counted++;
            }
            const insiderBps = Number((insiderSum * BigInt(10000)) / supplyRaw);
            const insiderPct = insiderBps / 100;
            console.log(`📊 Supply Analysis: ${supplyRaw.toString()} tokens. Insiders: ${insiderPct.toFixed(2)}%`);
            if (insiderPct > 20) {
                console.log(`⚠️ CABAL DETECTED: Insiders hold ${insiderPct.toFixed(2)}%`);
                return false;
            }
            return true;
        }
        catch (e) {
            console.error("Cabal check failed:", e);
            return true;
        }
    }
    async processMigrationLog(signature, slot) {
        try {
            // Try to parse from logs only (fast path) or fall back to transaction fetch
            let messageKeys = null;
            const tx = await this.connection.getTransaction(signature, {
                maxSupportedTransactionVersion: 0,
                commitment: "confirmed"
            });
            if (!tx) {
                console.warn(`Transaction not found: ${signature}`);
                return;
            }
            // Extract account keys
            const msg = tx.transaction.message;
            const keyObjs = "getAccountKeys" in msg
                ? msg.getAccountKeys().staticAccountKeys
                : msg.accountKeys;
            messageKeys = keyObjs.map((k) => typeof k === "string" ? k : k.toBase58());
            // Step 2: Trap Check - Verify signer is PUMP_MIGRATION_AUTH
            const hasAuth = tx.transaction.signatures.some((s) => s.publicKey?.toBase58
                ? s.publicKey.toBase58() === config_1.PUMP_MIGRATION_AUTH
                : s === config_1.PUMP_MIGRATION_AUTH);
            if (!hasAuth) {
                return;
            }
            // Step 3: Parse Pool Keys inline from account order (Raydium initialize2)
            if (!messageKeys) {
                console.warn("Could not extract message keys");
                return;
            }
            const poolKeys = this.parsePoolKeysFromTx(messageKeys);
            if (!poolKeys) {
                console.warn(`Could not parse pool keys from transaction`);
                return;
            }
            const mint = poolKeys.baseMint.toBase58() === "So11111111111111111111111111111111111111112"
                ? poolKeys.quoteMint.toBase58()
                : poolKeys.baseMint.toBase58();
            // Whitelist enforcement: if provided, only fire if present then delete to prevent duplicates
            if (this.whitelist) {
                const entry = this.whitelist.consume(mint);
                if (!entry) {
                    return;
                }
            }
            console.log(`✨ NEW MIGRATION DETECTED: ${mint}`);
            console.log(`🔍 Running Paranoia Protocols for ${mint}...`);
            // Quick mint authority check to avoid mintable rugs
            try {
                const mintInfo = await this.connection.getAccountInfo(new web3_js_1.PublicKey(mint), "processed");
                if (mintInfo && mintInfo.data?.length >= 82) {
                    const mintData = mintInfo.data;
                    const hasMintAuthority = mintData.readUInt32LE(0) !== 0;
                    const hasFreezeAuthority = mintData.readUInt32LE(46) !== 0;
                    if (hasMintAuthority || hasFreezeAuthority) {
                        console.log(`⛔ ABORT: Mint or freeze authority still enabled for ${mint}`);
                        return;
                    }
                }
            }
            catch (e) {
                console.warn(`⚠️ Mint authority check failed for ${mint}:`, e);
            }
            let metadata = {
                name: "Unknown",
                description: "",
                symbol: "???",
                twitter: undefined,
            };
            if (config_1.RELAX_FILTERS) {
                console.warn("⚠️ RELAX_FILTERS enabled – skipping social + cabal checks for testing.");
            }
            if (!config_1.RELAX_FILTERS) {
                // PARANOIA PROTOCOLS: Sequential Execution for Rate Limit Safety (Free Tier)
                // We run these one by one to avoid hitting the 10 RPS limit
                const isCabalSafe = await this.checkCabal(mint);
                if (!isCabalSafe) {
                    console.log(`⛔ ABORT: Cabal detected (Top 10 > 20%) for ${mint}`);
                    return;
                }
                const metadataResult = await this.checkMetadata(mint);
                if (!metadataResult.safe) {
                    console.log(`⛔ ABORT: No Socials (Twitter/TG) found for ${mint}`);
                    return;
                }
                metadata = metadataResult.data;
                console.log(`✅ PASS: Token survived Paranoia Protocols. Engaging.`);
            }
            else {
                console.log(`✅ RELAX MODE: Forcing buy pipeline for ${mint}.`);
            }
            // Step 4: Execute snipe
            const buySignal = {
                mint,
                name: metadata.name,
                description: metadata.description,
                twitterHandle: metadata.twitter,
                poolKeys: poolKeys
            };
            const bundleId = await this.sniperEngine.buy(buySignal);
            if (bundleId) {
                // Spawn AI analysis asynchronously (don't block the listener)
                this.sentientBrain
                    .analyzeToken(mint, {
                    name: metadata.name,
                    symbol: metadata.symbol,
                    description: metadata.description,
                    twitter: metadata.twitter,
                })
                    .then((score) => {
                    this.sentientBrain.recordPosition(mint, score, "", poolKeys);
                });
            }
        }
        catch (err) {
            console.error("processMigrationLog failed:", err);
        }
    }
    async fetchTokenMetadata(mint) {
        try {
            // Use Helius DAS API
            const response = await axios_1.default.post(this.heliusRpcUrl, {
                jsonrpc: "2.0",
                id: 1,
                method: "getAsset",
                params: {
                    id: mint,
                },
            });
            const asset = response.data.result;
            const hasTwitter = Boolean(asset?.extensions?.twitter ||
                asset?.content?.metadata?.social?.twitter);
            const hasTelegram = Boolean(asset?.extensions?.telegram ||
                asset?.content?.metadata?.social?.telegram);
            return {
                name: asset?.content?.metadata?.name || "Unknown",
                symbol: asset?.content?.metadata?.symbol || "???",
                description: asset?.content?.metadata?.description || "",
                twitter: asset?.extensions?.twitter,
                hasTwitter,
                hasTelegram,
            };
        }
        catch (err) {
            console.error(`Failed to fetch metadata for ${mint}:`, err);
            return {
                name: "Unknown",
                symbol: "???",
                description: "",
                hasTwitter: false,
                hasTelegram: false,
            };
        }
    }
}
exports.MigrationListener = MigrationListener;
//# sourceMappingURL=MigrationListener.js.map