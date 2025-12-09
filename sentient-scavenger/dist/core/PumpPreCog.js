"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.PumpPreCog = void 0;
const web3_js_1 = require("@solana/web3.js");
const axios_1 = __importDefault(require("axios"));
const config_1 = require("../config");
/**
 * Producer: watches Pump.fun, prefilters, runs heavy checks, and populates the whitelist.
 * This keeps heavy work off the Raydium hot path.
 */
class PumpPreCog {
    constructor(connection, heliusRpcUrl, whitelist) {
        this.logCount = 0;
        this.connection = connection;
        this.heliusRpcUrl = heliusRpcUrl;
        this.whitelist = whitelist;
    }
    getVelocity() {
        return this.logCount;
    }
    async start() {
        console.log("🧠 PumpPreCog: Watching Pump.fun for near-complete curves...");
        try {
            const programId = new web3_js_1.PublicKey(config_1.PUMP_PROGRAM);
            this.connection.onLogs(programId, async (logs) => {
                this.logCount++;
                if (this.logCount % 1000 === 0) {
                    console.log(`⚡ PumpPreCog Velocity: Saw ${this.logCount} Pump.fun logs.`);
                }
                const logsStr = logs.logs.join(" ");
                const nearComplete = this.isNearComplete(logsStr);
                // Light prefilter: only react to potential completion/bonding curve logs
                if (!nearComplete && !config_1.RELAX_FILTERS)
                    return;
                const mint = this.extractMintFromLogs(logsStr);
                if (!mint)
                    return;
                if (nearComplete) {
                    console.log(`🧲 PumpPreCog candidate: ${mint}`);
                }
                else if (config_1.RELAX_FILTERS) {
                    // In relax mode we still allow through for debugging so we can see inserts
                    console.log(`🧲 PumpPreCog (RELAX, no curve marker) candidate: ${mint}`);
                }
                const candidate = { mint, decimals: config_1.DEFAULT_TOKEN_DECIMALS }; // Pump.fun tokens are typically 6 decimals
                if (config_1.RELAX_FILTERS) {
                    this.whitelist.upsert(candidate);
                    console.log(`🟢 RELAX: Whitelisted ${mint} (TTL refresh)`);
                    return;
                }
                const [socialResult, cabalSafe, mintSafe] = await Promise.all([
                    this.checkSocials(candidate.mint),
                    this.checkCabal(candidate.mint),
                    this.checkMintAuthorities(candidate.mint),
                ]);
                if (!socialResult.hasSocials) {
                    console.log(`⛔ PumpPreCog: Missing socials for ${mint}`);
                    return;
                }
                if (!cabalSafe) {
                    console.log(`⛔ PumpPreCog: Cabal >${config_1.MAX_CABAL_PCT}% for ${mint}`);
                    return;
                }
                if (!mintSafe) {
                    console.log(`⛔ PumpPreCog: Mint/freeze authority enabled for ${mint}`);
                    return;
                }
                const decimals = socialResult.decimals ?? config_1.DEFAULT_TOKEN_DECIMALS;
                this.whitelist.upsert({ mint, decimals });
                console.log(`✅ PumpPreCog: Whitelisted ${mint} (decimals=${decimals})`);
            }, "processed");
        }
        catch (err) {
            console.error("PumpPreCog failed to start:", err);
        }
    }
    isNearComplete(logsStr) {
        // Heuristic: look for common completion markers to avoid API burn.
        const lowered = logsStr.toLowerCase();
        const pctMatch = lowered.match(/(\d{2,3})%/);
        if (pctMatch) {
            const pct = parseInt(pctMatch[1], 10);
            if (pct >= config_1.MIN_CURVE_PREFILTER)
                return true;
        }
        return (lowered.includes("complete") ||
            lowered.includes("bonding") ||
            lowered.includes("tradeevent"));
    }
    extractMintFromLogs(logsStr) {
        // Strategy 1: Look for "Mint: <base58>" pattern (Common in program logs)
        const mintMatch = logsStr.match(/Mint: ([a-zA-Z0-9]{32,44})/);
        if (mintMatch)
            return mintMatch[1];
        // Strategy 2: Look for any token ending in "pump"
        const parts = logsStr.split(/[^a-zA-Z0-9]+/); // Split by non-alphanumeric to isolate words
        const candidate = parts.find((p) => p.length >= 32 && p.length <= 44 && p.endsWith("pump"));
        if (!candidate) {
            // Verbose logging for debugging (Sampled)
            if (Math.random() < 0.01) {
                console.log(`⚠️ PumpPreCog: No mint found in log sample: ${logsStr.substring(0, 150)}...`);
            }
        }
        return candidate || null;
    }
    async checkSocials(mint) {
        try {
            const response = await axios_1.default.post(this.heliusRpcUrl, {
                jsonrpc: "2.0",
                id: "social-check",
                method: "getAsset",
                params: { id: mint },
            });
            const asset = response.data?.result;
            const hasTwitter = Boolean(asset?.extensions?.twitter || asset?.content?.metadata?.social?.twitter);
            const hasTelegram = Boolean(asset?.extensions?.telegram || asset?.content?.metadata?.social?.telegram);
            const hasJsonUri = Boolean(asset?.content?.json_uri);
            const decimals = asset?.content?.metadata?.decimals;
            return { hasSocials: hasJsonUri && (hasTwitter || hasTelegram), decimals };
        }
        catch (e) {
            console.warn("Social check failed:", e);
            return { hasSocials: false };
        }
    }
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
            if (supplyRaw === BigInt(0))
                return true;
            const sorted = largestAccounts.value
                .map((acc) => ({ amount: BigInt(acc.amount) }))
                .sort((a, b) => Number(b.amount - a.amount));
            let insiderSum = BigInt(0);
            for (let i = 0, counted = 0; i < sorted.length && counted < 10; i++) {
                const pct = (sorted[i].amount * BigInt(10000)) / supplyRaw;
                // skip likely pool/curve accounts >30%
                if (pct > BigInt(3000))
                    continue;
                insiderSum += sorted[i].amount;
                counted++;
            }
            const insiderPct = Number((insiderSum * BigInt(10000)) / supplyRaw) / 100;
            return insiderPct <= config_1.MAX_CABAL_PCT;
        }
        catch (e) {
            console.warn("Cabal check failed:", e);
            return false;
        }
    }
    async checkMintAuthorities(mint) {
        try {
            const info = await this.connection.getAccountInfo(new web3_js_1.PublicKey(mint), "processed");
            if (!info || !info.data || info.data.length < 82)
                return false;
            const data = info.data;
            const hasMintAuthority = data.readUInt32LE(0) !== 0;
            const hasFreezeAuthority = data.readUInt32LE(46) !== 0;
            return !hasMintAuthority && !hasFreezeAuthority;
        }
        catch (e) {
            console.warn("Mint authority check failed:", e);
            return false;
        }
    }
}
exports.PumpPreCog = PumpPreCog;
//# sourceMappingURL=PumpPreCog.js.map