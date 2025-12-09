"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.Janitor = void 0;
const web3_js_1 = require("@solana/web3.js");
const spl_token_1 = require("@solana/spl-token");
const config_1 = require("../config");
class Janitor {
    constructor(connection, keypair) {
        this.connection = connection;
        this.keypair = keypair;
    }
    async cleanupEmptyTokenAccounts() {
        try {
            // console.log("🧹 Janitor: Scanning for empty token accounts..."); // Silenced to reduce noise
            const payer = this.keypair.publicKey;
            const accounts = await this.connection.getParsedTokenAccountsByOwner(payer, { programId: new web3_js_1.PublicKey(config_1.TOKEN_PROGRAM) });
            let recovered = 0;
            for (const { pubkey, account } of accounts.value) {
                const parsed = account.data;
                const amount = parsed.parsed?.info?.tokenAmount?.uiAmount || 0;
                if (amount === 0) {
                    console.log(`  └─ Closing empty account: ${pubkey.toBase58()}`);
                    const closeIx = (0, spl_token_1.createCloseAccountInstruction)(pubkey, payer, payer);
                    if (config_1.DRY_RUN) {
                        console.log(`     (DRY RUN) Would reclaim ~0.002 SOL`);
                        recovered++;
                    }
                    else {
                        try {
                            const tx = new web3_js_1.Transaction().add(closeIx);
                            await (0, web3_js_1.sendAndConfirmTransaction)(this.connection, tx, [this.keypair]);
                            console.log(`     ✅ Reclaimed rent from ${pubkey.toBase58()}`);
                            recovered++;
                        }
                        catch (e) {
                            console.error(`     ❌ Failed to close ${pubkey.toBase58()}:`, e);
                        }
                    }
                }
            }
            if (recovered > 0) {
                console.log(`✅ Janitor recovered rent from ${recovered} empty accounts`);
            }
            else {
                // console.log(`✅ Janitor: No empty accounts to cleanup`); // Silenced
            }
        }
        catch (err) {
            if (err.message?.includes("fetch failed") || err.code === "ETIMEDOUT") {
                console.warn("⚠️ Janitor network timeout (skipping this cycle)");
            }
            else {
                console.error("Janitor failed:", err);
            }
        }
    }
    async startMaintenanceLoop(intervalMs) {
        console.log(`🔄 Janitor loop started (interval: ${intervalMs}ms)`);
        setInterval(() => {
            this.cleanupEmptyTokenAccounts().catch((err) => {
                console.error("Janitor loop error:", err);
            });
        }, intervalMs);
    }
}
exports.Janitor = Janitor;
//# sourceMappingURL=Janitor.js.map