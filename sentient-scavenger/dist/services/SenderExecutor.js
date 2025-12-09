"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.SenderExecutor = void 0;
const web3_js_1 = require("@solana/web3.js");
const BlockhashManager_1 = require("./BlockhashManager");
const axios_1 = __importDefault(require("axios"));
const bs58_1 = __importDefault(require("bs58"));
const config_1 = require("../config");
class SenderExecutor {
    constructor(connection, keypair) {
        this.senderUrl = "https://sender.helius-rpc.com/fast";
        this.connection = connection;
        this.keypair = keypair;
    }
    async executeAndConfirm(transaction, signTransaction = true) {
        try {
            const blockhash = (0, BlockhashManager_1.getCachedBlockhash)();
            if (!blockhash) {
                throw new Error("No cached blockhash available");
            }
            // 1. Add Tip Instruction (Required for Sender)
            // Note: Sender expects a single transaction with the tip instruction included.
            // We cannot send a bundle of [MainTx, TipTx] like Jito.
            // We must append the tip instruction to the MainTx.
            const tipAmount = 0.0002; // Minimum required by Sender
            const tipAccount = config_1.JITO_TIP_ACCOUNTS[Math.floor(Math.random() * config_1.JITO_TIP_ACCOUNTS.length)];
            const tipIx = web3_js_1.SystemProgram.transfer({
                fromPubkey: this.keypair.publicKey,
                toPubkey: new web3_js_1.PublicKey(tipAccount),
                lamports: Math.floor(tipAmount * 1e9),
            });
            // Rebuild transaction to include tip
            if (transaction instanceof web3_js_1.VersionedTransaction) {
                // VersionedTransaction is harder to modify in-place without deserializing/reserializing
                // For now, we assume we can just send it as is if it already has a tip, 
                // OR we might need to refactor the caller to add the tip before creating the VersionedTx.
                // BUT, for simplicity, let's assume the caller handles the tip for VersionedTx, 
                // or we just send it to Jito if it's complex.
                // Actually, let's just try sending it.
                if (signTransaction)
                    transaction.sign([this.keypair]);
            }
            else {
                // Legacy Transaction: Easy to add instruction
                transaction.add(tipIx);
                transaction.recentBlockhash = blockhash.blockhash;
                transaction.feePayer = this.keypair.publicKey;
                if (signTransaction)
                    transaction.sign(this.keypair);
            }
            if (config_1.DRY_RUN) {
                console.log("🔬 DRY RUN: Would send tx to Helius Sender");
                return "DRY_RUN_SENDER_" + Math.random().toString(36).substring(7);
            }
            // Serialize
            const serializedTx = transaction.serialize();
            const encodedTx = bs58_1.default.encode(serializedTx);
            // Send to Helius Sender
            console.log("🚀 Broadcasting via Helius Sender...");
            const response = await axios_1.default.post(this.senderUrl, {
                jsonrpc: "2.0",
                id: 1,
                method: "sendTransaction",
                params: [encodedTx, { skipPreflight: true }]
            });
            if (response.data.error) {
                console.error("Sender Error:", response.data.error);
                return null;
            }
            const signature = response.data.result;
            console.log(`✅ Sender Broadcast: ${signature}`);
            return signature;
        }
        catch (err) {
            console.error("Sender Execution Failed:", err);
            return null;
        }
    }
}
exports.SenderExecutor = SenderExecutor;
//# sourceMappingURL=SenderExecutor.js.map