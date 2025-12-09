"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.JitoExecutor = void 0;
const web3_js_1 = require("@solana/web3.js");
const BlockhashManager_1 = require("./BlockhashManager");
const axios_1 = __importDefault(require("axios"));
const bs58_1 = __importDefault(require("bs58"));
const config_1 = require("../config");
class JitoExecutor {
    constructor(connection, keypair) {
        this.bundleStatuses = new Map();
        this.bundleWaiters = new Map();
        this.connection = connection;
        this.keypair = keypair;
    }
    async executeAndConfirm(transaction, signTransaction = true) {
        try {
            const blockhash = (0, BlockhashManager_1.getCachedBlockhash)();
            if (!blockhash) {
                throw new Error("No cached blockhash available");
            }
            // Sign the main transaction if needed
            if (signTransaction) {
                if (transaction instanceof web3_js_1.VersionedTransaction) {
                    transaction.sign([this.keypair]);
                }
                else {
                    transaction.recentBlockhash = blockhash.blockhash;
                    transaction.sign(this.keypair);
                }
            }
            // Create separate Tip Transaction
            const tipAmount = this.calculateDynamicTip();
            const tipAccount = config_1.JITO_TIP_ACCOUNTS[Math.floor(Math.random() * config_1.JITO_TIP_ACCOUNTS.length)];
            // console.log(`DEBUG: Using Tip Account: ${tipAccount}`);
            const tipTx = new web3_js_1.Transaction();
            tipTx.add(web3_js_1.SystemProgram.transfer({
                fromPubkey: this.keypair.publicKey,
                toPubkey: new web3_js_1.PublicKey(tipAccount),
                lamports: Math.floor(tipAmount * 1e9),
            }));
            tipTx.recentBlockhash = blockhash.blockhash;
            tipTx.feePayer = this.keypair.publicKey;
            tipTx.sign(this.keypair);
            if (config_1.DRY_RUN) {
                console.log("🔬 DRY RUN: Would send bundle to Jito");
                console.log("  Tip Account: " + tipAccount);
                console.log("  Tip Amount: " + tipAmount.toFixed(4) + " SOL");
                return "DRY_RUN_BUNDLE_" + Math.random().toString(36).substring(7);
            }
            // Serialize both transactions
            const encodedTx = bs58_1.default.encode(transaction instanceof web3_js_1.VersionedTransaction
                ? transaction.serialize()
                : transaction.serialize());
            const encodedTipTx = bs58_1.default.encode(tipTx.serialize());
            // Send bundle [MainTx, TipTx]
            console.log("📤 Submitting bundle to Jito...");
            const response = await axios_1.default.post(config_1.JITO_BLOCK_ENGINE_URL, {
                jsonrpc: "2.0",
                id: 1,
                method: "sendBundle",
                params: [[encodedTx, encodedTipTx]],
            });
            if (response.data.error) {
                throw new Error(`Jito error: ${response.data.error.message}`);
            }
            const bundleId = response.data.result;
            console.log("✅ Bundle sent to Jito: " + bundleId);
            // Start async confirmation polling (don't wait, return immediately)
            this.pollBundleConfirmation(bundleId).catch((err) => {
                console.error("Bundle confirmation polling failed:", err);
            });
            return bundleId;
        }
        catch (err) {
            console.error("JitoExecutor.executeAndConfirm failed:", err);
            return null;
        }
    }
    /**
     * Poll Jito for bundle confirmation status
     * This runs async in the background
     */
    async pollBundleConfirmation(bundleId) {
        const startTime = Date.now();
        let attempts = 0;
        const maxAttempts = Math.floor(config_1.BUNDLE_CONFIRMATION_TIMEOUT / config_1.BUNDLE_CONFIRMATION_POLL);
        console.log(`⏳ Polling bundle status (timeout: ${config_1.BUNDLE_CONFIRMATION_TIMEOUT / 1000}s)...`);
        while (attempts < maxAttempts) {
            try {
                const response = await axios_1.default.post(config_1.JITO_BLOCK_ENGINE_URL, {
                    jsonrpc: "2.0",
                    id: 1,
                    method: "getBundleStatuses",
                    params: [[bundleId]],
                });
                if (response.data.result && response.data.result.value && response.data.result.value.length > 0) {
                    const status = response.data.result.value[0];
                    if (status && (status.confirmation_status === "confirmed" || status.confirmation_status === "finalized")) {
                        console.log(`✅ Bundle CONFIRMED in slot ${status.slot}: ${bundleId}`);
                        this.bundleStatuses.set(bundleId, {
                            bundleId,
                            confirmed: true,
                            slot: status.slot,
                        });
                        this.resolveBundleWaiters(bundleId);
                        return;
                    }
                }
                attempts++;
                if (attempts % 5 === 0) {
                    console.log(`  └─ Still pending (attempt ${attempts}/${maxAttempts})`);
                }
                // Wait before next poll
                await new Promise((resolve) => setTimeout(resolve, config_1.BUNDLE_CONFIRMATION_POLL));
            }
            catch (err) {
                console.warn(`Bundle status check failed (attempt ${attempts}):`, err);
                attempts++;
                await new Promise((resolve) => setTimeout(resolve, config_1.BUNDLE_CONFIRMATION_POLL * 2)); // Back off on error
            }
        }
        // Timeout reached
        console.error(`❌ Bundle confirmation timeout: ${bundleId} (waited ${config_1.BUNDLE_CONFIRMATION_TIMEOUT / 1000}s)`);
        this.bundleStatuses.set(bundleId, {
            bundleId,
            confirmed: false,
            error: "Confirmation timeout",
        });
        this.resolveBundleWaiters(bundleId);
    }
    /**
     * Get bundle status (if polling already completed)
     */
    getBundleStatus(bundleId) {
        return this.bundleStatuses.get(bundleId);
    }
    /**
     * Await bundle confirmation result (resolves when poller finishes)
     */
    async waitForBundleStatus(bundleId, timeoutMs = config_1.BUNDLE_CONFIRMATION_TIMEOUT + 5000) {
        // Immediate success for Dry Run
        if (bundleId.startsWith("DRY_RUN_BUNDLE")) {
            return {
                bundleId,
                confirmed: true,
                slot: 0,
            };
        }
        const existing = this.bundleStatuses.get(bundleId);
        if (existing)
            return existing;
        return new Promise((resolve, reject) => {
            const timer = setTimeout(() => {
                this.bundleWaiters.delete(bundleId);
                reject(new Error(`Bundle status timeout for ${bundleId} after ${timeoutMs}ms`));
            }, timeoutMs);
            const handler = (status) => {
                clearTimeout(timer);
                resolve(status);
            };
            const waiters = this.bundleWaiters.get(bundleId) || [];
            waiters.push(handler);
            this.bundleWaiters.set(bundleId, waiters);
        });
    }
    resolveBundleWaiters(bundleId) {
        const status = this.bundleStatuses.get(bundleId);
        const waiters = this.bundleWaiters.get(bundleId);
        if (!status || !waiters)
            return;
        waiters.forEach((cb) => cb(status));
        this.bundleWaiters.delete(bundleId);
    }
    /**
     * Calculate dynamic Jito tip
     * Spec: min(0.005 SOL, 1% of wagered amount), floor at 0.0005 SOL
     * Potential profit assumed to be 100% of wager (2x)
     */
    calculateDynamicTip() {
        const profitEstimate = config_1.SOL_WAGER_AMOUNT; // Assume 100% gain
        const onePercent = profitEstimate * 0.01;
        const cap = 0.005;
        const floor = 0.0005;
        let tip = Math.min(cap, onePercent);
        tip = Math.max(floor, tip);
        // Respect optional env cap JITO_TIP_CAP if set lower
        return Math.min(tip, (process.env.JITO_TIP_CAP && parseFloat(process.env.JITO_TIP_CAP)) || tip);
    }
    /**
     * Retry failed bundle (fire again)
     */
    async retryBundle(transaction) {
        console.log("🔄 Retrying bundle with exponential backoff...");
        // TODO: Implement with exponential backoff
        return this.executeAndConfirm(transaction);
    }
}
exports.JitoExecutor = JitoExecutor;
//# sourceMappingURL=JitoExecutor.js.map