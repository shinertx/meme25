"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.SniperEngine = void 0;
const web3_js_1 = require("@solana/web3.js");
const spl_token_1 = require("@solana/spl-token");
const web3_js_2 = require("@solana/web3.js");
const BlockhashManager_1 = require("../services/BlockhashManager");
const config_1 = require("../config");
const raydium_1 = require("../utils/raydium");
class SniperEngine {
    constructor(connection, keypair, jitoExecutor, senderExecutor) {
        this.lastEntryPrice = new Map();
        this.poolKeysMap = new Map();
        this.connection = connection;
        this.keypair = keypair;
        this.jitoExecutor = jitoExecutor;
        this.senderExecutor = senderExecutor;
    }
    async buy(signal) {
        try {
            console.log(`🎯 SNIPE SIGNAL: ${signal.name} (${signal.mint})`);
            this.poolKeysMap.set(signal.mint, signal.poolKeys);
            const amountIn = (0, raydium_1.solToLamports)(config_1.SOL_WAGER_AMOUNT);
            // Calculate slippage locally
            const { minAmountOut: minAmountOutBN } = await (0, raydium_1.calculateAmountOut)(this.connection, signal.poolKeys, amountIn, config_1.SLIPPAGE_BPS, new web3_js_1.PublicKey(config_1.WSOL_MINT));
            const minAmountOut = minAmountOutBN.toNumber();
            // 1. Setup Token Accounts
            const wsolAta = await (0, spl_token_1.getAssociatedTokenAddress)(new web3_js_1.PublicKey(config_1.WSOL_MINT), this.keypair.publicKey);
            const tokenAta = await (0, spl_token_1.getAssociatedTokenAddress)(new web3_js_1.PublicKey(signal.mint), this.keypair.publicKey);
            const instructions = [];
            // Optional compute budget bump for reliability
            instructions.push(web3_js_2.ComputeBudgetProgram.setComputeUnitLimit({ units: 100000 }));
            // 0. Ensure WSOL ATA exists and is funded
            const wsolPrep = await this.prepareWsolAccount(wsolAta, amountIn);
            instructions.push(...wsolPrep);
            // 2. Create Token ATA (Destination)
            instructions.push((0, spl_token_1.createAssociatedTokenAccountIdempotentInstruction)(this.keypair.publicKey, tokenAta, this.keypair.publicKey, new web3_js_1.PublicKey(signal.mint)));
            // 3. Build Swap Instruction
            const swapIx = await (0, raydium_1.buildSwapInstruction)(this.connection, signal.poolKeys, this.keypair.publicKey, wsolAta, // Input (WSOL)
            tokenAta, // Output (Token)
            amountIn, minAmountOut);
            instructions.push(swapIx);
            // 5. Build Transaction
            const blockhash = (0, BlockhashManager_1.getCachedBlockhash)();
            if (!blockhash)
                throw new Error("No blockhash");
            const messageV0 = new web3_js_1.TransactionMessage({
                payerKey: this.keypair.publicKey,
                recentBlockhash: blockhash.blockhash,
                instructions,
            }).compileToV0Message();
            const transaction = new web3_js_1.VersionedTransaction(messageV0);
            transaction.sign([this.keypair]);
            // Send to Jito AND Sender (Dual Routing)
            console.log(`  └─ Submitting to Jito + Sender (Dual Routing)...`);
            // We fire both and race them, but we only track one for confirmation logic for now.
            // Sender is faster but Jito has better revert protection.
            // Since Sender also routes to Jito, we might just use Sender primarily if we trust it.
            // But let's do both for maximum coverage.
            const jitoPromise = this.jitoExecutor.executeAndConfirm(transaction);
            const senderPromise = this.senderExecutor.executeAndConfirm(transaction);
            const [bundleId, senderSig] = await Promise.all([jitoPromise, senderPromise]);
            if (senderSig) {
                console.log(`✅ Sender accepted tx: ${senderSig}`);
                // We could return early here if we trust Sender's immediate response
            }
            if (bundleId) {
                console.log(`⏳ Waiting for bundle confirmation: ${bundleId}`);
                try {
                    const status = await this.jitoExecutor.waitForBundleStatus(bundleId);
                    if (status.confirmed) {
                        console.log(`✅ Bundle confirmed for ${signal.mint}: ${bundleId}`);
                        // Calculate entry price
                        const tokenDecimals = signal.poolKeys.baseDecimals;
                        const tokensReceived = minAmountOut / 10 ** tokenDecimals;
                        const entryPrice = config_1.SOL_WAGER_AMOUNT / tokensReceived;
                        this.lastEntryPrice.set(signal.mint, entryPrice);
                        console.log(`� Entry Price: ${entryPrice.toFixed(9)} SOL (Est. Tokens: ${tokensReceived})`);
                        return bundleId;
                    }
                    else {
                        console.warn(`⚠️ Bundle NOT confirmed for ${signal.mint}: ${status.error || "unknown"}`);
                        return null; // Do not record position
                    }
                }
                catch (err) {
                    console.error(`Bundle status wait failed for ${bundleId}:`, err);
                    return null;
                }
            }
            else if (config_1.DRY_RUN) {
                this.lastEntryPrice.set(signal.mint, 1.0);
                console.log(`✅ (DRY RUN) Buy would execute`);
                return "DRY_RUN_BUNDLE_ID";
            }
            return null;
        }
        catch (err) {
            console.error("SniperEngine.buy() failed:", err);
            return null;
        }
    }
    async sell(mint, amount, isEmergency = false) {
        try {
            console.log(`💰 SELLING: ${mint} ${isEmergency ? "(PANIC SELL - 100% SLIPPAGE)" : ""}`);
            const poolKeys = this.poolKeysMap.get(mint);
            if (!poolKeys) {
                console.error(`❌ No pool keys found for ${mint} - cannot sell locally`);
                return null;
            }
            const amountIn = amount; // Raw amount
            // If emergency, 100% slippage (minAmountOut = 0)
            let minAmountOut = 0;
            if (!isEmergency) {
                const { minAmountOut: minOutBN } = await (0, raydium_1.calculateAmountOut)(this.connection, poolKeys, amountIn, config_1.SLIPPAGE_BPS, new web3_js_1.PublicKey(mint));
                minAmountOut = minOutBN.toNumber();
            }
            // 1. Setup Token Accounts
            const wsolAta = await (0, spl_token_1.getAssociatedTokenAddress)(new web3_js_1.PublicKey(config_1.WSOL_MINT), this.keypair.publicKey);
            const tokenAta = await (0, spl_token_1.getAssociatedTokenAddress)(new web3_js_1.PublicKey(mint), this.keypair.publicKey);
            const instructions = [];
            // 2. Build Swap Instruction (Token -> SOL)
            const swapIx = await (0, raydium_1.buildSwapInstruction)(this.connection, poolKeys, this.keypair.publicKey, tokenAta, // Input (Token)
            wsolAta, // Output (WSOL)
            amountIn, minAmountOut);
            instructions.push(swapIx);
            // 3. Close WSOL account to unwrap SOL (optional, or keep as wSOL)
            // For now, we keep as wSOL to save gas/time, or unwrap if needed.
            // Let's unwrap to realize profit in SOL.
            // instructions.push(createCloseAccountInstruction(wsolAta, this.keypair.publicKey, this.keypair.publicKey));
            // 4. Build Transaction
            const blockhash = (0, BlockhashManager_1.getCachedBlockhash)();
            if (!blockhash)
                throw new Error("No blockhash");
            const messageV0 = new web3_js_1.TransactionMessage({
                payerKey: this.keypair.publicKey,
                recentBlockhash: blockhash.blockhash,
                instructions,
            }).compileToV0Message();
            const transaction = new web3_js_1.VersionedTransaction(messageV0);
            transaction.sign([this.keypair]);
            // Send to Jito
            console.log(`  └─ Submitting sell bundle to Jito...`);
            const bundleId = await this.jitoExecutor.executeAndConfirm(transaction);
            if (bundleId) {
                console.log(`✅ Sell executed: ${bundleId}`);
                this.jitoExecutor
                    .waitForBundleStatus(bundleId)
                    .then((status) => {
                    if (status.confirmed) {
                        console.log(`📦 Sell bundle confirmed for ${mint}: ${bundleId}`);
                    }
                    else {
                        console.warn(`⚠️ Sell bundle NOT confirmed for ${mint}: ${status.error || "unknown"}`);
                    }
                })
                    .catch((err) => {
                    console.error(`Sell bundle status wait failed for ${bundleId}:`, err);
                });
                return bundleId;
            }
            else if (config_1.DRY_RUN) {
                console.log(`✅ (DRY RUN) Sell would execute`);
                return "DRY_RUN_SELL_ID";
            }
            return null;
        }
        catch (err) {
            console.error("SniperEngine.sell() failed:", err);
            return null;
        }
    }
    /**
     * Get entry price for position tracking
     */
    getEntryPrice(mint) {
        return this.lastEntryPrice.get(mint) || 0;
    }
    /**
     * Register pool keys manually (e.g. from persistence)
     */
    registerPoolKeys(mint, keys) {
        this.poolKeysMap.set(mint, keys);
    }
    async prepareWsolAccount(wsolAta, requiredLamports) {
        const instructions = [];
        const needed = BigInt(requiredLamports);
        let current = BigInt(0);
        let accountExists = false;
        try {
            const balance = await this.connection.getTokenAccountBalance(wsolAta);
            if (balance?.value?.amount) {
                current = BigInt(balance.value.amount);
                accountExists = true;
            }
        }
        catch {
            accountExists = false;
        }
        if (!accountExists) {
            instructions.push((0, spl_token_1.createAssociatedTokenAccountIdempotentInstruction)(this.keypair.publicKey, wsolAta, this.keypair.publicKey, new web3_js_1.PublicKey(config_1.WSOL_MINT)));
        }
        if (current < needed) {
            const diff = Number(needed - current);
            instructions.push(web3_js_1.SystemProgram.transfer({
                fromPubkey: this.keypair.publicKey,
                toPubkey: wsolAta,
                lamports: diff,
            }), (0, spl_token_1.createSyncNativeInstruction)(wsolAta));
        }
        return instructions;
    }
}
exports.SniperEngine = SniperEngine;
//# sourceMappingURL=SniperEngine.js.map