"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.parseRaydiumMigration = parseRaydiumMigration;
const web3_js_1 = require("@solana/web3.js");
const raydium_sdk_1 = require("@raydium-io/raydium-sdk");
const config_1 = require("../config");
async function parseRaydiumMigration(connection, tx) {
    try {
        // Find the instruction that calls Raydium V4 initialize2
        let ix = null;
        // 1. Check top-level instructions
        for (const instruction of tx.transaction.message.instructions) {
            if (isRaydiumInitialize2(instruction)) {
                ix = instruction;
                break;
            }
        }
        // 2. Check inner instructions
        if (!ix && tx.meta?.innerInstructions) {
            for (const inner of tx.meta.innerInstructions) {
                for (const instruction of inner.instructions) {
                    if (isRaydiumInitialize2(instruction)) {
                        ix = instruction;
                        break;
                    }
                }
                if (ix)
                    break;
            }
        }
        if (!ix)
            return null;
        return await extractPoolKeys(connection, ix, tx);
    }
    catch (err) {
        console.error("parseRaydiumMigration failed:", err);
        return null;
    }
}
function isRaydiumInitialize2(ix) {
    if (ix.programId.toString() !== config_1.RAYDIUM_V4_PROGRAM)
        return false;
    if ('data' in ix) {
        const data = require('bs58').decode(ix.data);
        return data[0] === 1; // initialize2 discriminator
    }
    return false;
}
async function extractPoolKeys(connection, ix, tx) {
    const accounts = ix.accounts;
    if (accounts.length < 16)
        return null;
    const id = accounts[3];
    const authority = accounts[4];
    const openOrders = accounts[5];
    const lpMint = accounts[6];
    const baseMint = accounts[7];
    const quoteMint = accounts[8];
    const baseVault = accounts[9];
    const quoteVault = accounts[10];
    const targetOrders = accounts[11];
    const marketProgramId = accounts[14];
    const marketId = accounts[15];
    // Fetch Market Data to get bids/asks/eventQueue
    const marketInfo = await connection.getAccountInfo(marketId);
    if (!marketInfo) {
        console.error("Failed to fetch market info for " + marketId.toBase58());
        return null;
    }
    const marketState = raydium_sdk_1.MARKET_STATE_LAYOUT_V3.decode(marketInfo.data);
    const baseDecimals = findDecimals(tx, baseMint);
    const quoteDecimals = findDecimals(tx, quoteMint);
    const lpDecimals = findDecimals(tx, lpMint);
    return {
        id,
        baseMint,
        quoteMint,
        lpMint,
        baseDecimals,
        quoteDecimals,
        lpDecimals,
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
        marketAuthority: web3_js_1.PublicKey.default, // Derived by SDK?
        marketBaseVault: marketState.baseVault,
        marketQuoteVault: marketState.quoteVault,
        marketBids: marketState.bids,
        marketAsks: marketState.asks,
        marketEventQueue: marketState.eventQueue,
        lookupTableAccount: web3_js_1.PublicKey.default
    };
}
function findDecimals(tx, mint) {
    const mintStr = mint.toBase58();
    if (tx.meta?.preTokenBalances) {
        const found = tx.meta.preTokenBalances.find((b) => b.mint === mintStr);
        if (found && found.uiTokenAmount.decimals !== undefined)
            return found.uiTokenAmount.decimals;
    }
    if (tx.meta?.postTokenBalances) {
        const found = tx.meta.postTokenBalances.find((b) => b.mint === mintStr);
        if (found && found.uiTokenAmount.decimals !== undefined)
            return found.uiTokenAmount.decimals;
    }
    return 9;
}
//# sourceMappingURL=raydiumParser.js.map