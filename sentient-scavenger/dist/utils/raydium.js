"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.buildSwapInstruction = buildSwapInstruction;
exports.getPoolPrice = getPoolPrice;
exports.calculateAmountOut = calculateAmountOut;
exports.getRaydiumPoolInfo = getRaydiumPoolInfo;
exports.getTokenPrice = getTokenPrice;
exports.estimateSwapAmount = estimateSwapAmount;
exports.getSwapTransaction = getSwapTransaction;
exports.lamportsToSol = lamportsToSol;
exports.solToLamports = solToLamports;
const web3_js_1 = require("@solana/web3.js");
const raydium_sdk_1 = require("@raydium-io/raydium-sdk");
const spl_token_1 = require("@solana/spl-token");
const config_1 = require("../config");
const axios_1 = __importDefault(require("axios"));
const bn_js_1 = __importDefault(require("bn.js"));
/**
 * Build a Raydium Swap Instruction locally (No API calls)
 */
async function buildSwapInstruction(connection, poolKeys, userPublicKey, inputTokenAccount, outputTokenAccount, amountIn, // raw amount (lamports/atoms)
minAmountOut, // raw amount
fixedSide = "in") {
    try {
        const amountInBN = new bn_js_1.default(amountIn);
        const minAmountOutBN = new bn_js_1.default(minAmountOut);
        const baseToken = new raydium_sdk_1.Token(spl_token_1.TOKEN_PROGRAM_ID, poolKeys.baseMint, poolKeys.baseDecimals);
        const quoteToken = new raydium_sdk_1.Token(spl_token_1.TOKEN_PROGRAM_ID, poolKeys.quoteMint, poolKeys.quoteDecimals);
        const amountInToken = new raydium_sdk_1.TokenAmount(baseToken, amountInBN);
        const amountOutToken = new raydium_sdk_1.TokenAmount(quoteToken, minAmountOutBN);
        const { innerTransactions } = await raydium_sdk_1.Liquidity.makeSwapInstructionSimple({
            connection,
            poolKeys,
            userKeys: {
                tokenAccounts: [
                    // We don't need real balances for instruction generation, but we need correct owners
                    { pubkey: inputTokenAccount, programId: spl_token_1.TOKEN_PROGRAM_ID, accountInfo: { mint: poolKeys.baseMint, owner: userPublicKey, amount: new bn_js_1.default(0), delegateOption: 0, delegate: web3_js_1.PublicKey.default, state: 1, isNativeOption: 0, isNative: false, delegatedAmount: new bn_js_1.default(0), closeAuthorityOption: 0, closeAuthority: web3_js_1.PublicKey.default } },
                    { pubkey: outputTokenAccount, programId: spl_token_1.TOKEN_PROGRAM_ID, accountInfo: { mint: poolKeys.quoteMint, owner: userPublicKey, amount: new bn_js_1.default(0), delegateOption: 0, delegate: web3_js_1.PublicKey.default, state: 1, isNativeOption: 0, isNative: false, delegatedAmount: new bn_js_1.default(0), closeAuthorityOption: 0, closeAuthority: web3_js_1.PublicKey.default } }
                ],
                owner: userPublicKey,
            },
            amountIn: amountInToken,
            amountOut: amountOutToken,
            fixedSide: "in",
            makeTxVersion: raydium_sdk_1.TxVersion.V0,
            computeBudgetConfig: {
                microLamports: 100000, // Default, will be overridden by Jito bundle
                units: 400000
            },
            config: {
                bypassAssociatedCheck: true
            }
        });
        // Return the actual swap instruction (which contains remainingAccounts)
        const instructions = innerTransactions[0].instructions;
        const swapIx = instructions.find(ix => ix.programId.equals(new web3_js_1.PublicKey(config_1.RAYDIUM_V4_PROGRAM)));
        if (!swapIx) {
            // Fallback: It might be the last instruction if SDK structure changes
            return instructions[instructions.length - 1];
        }
        return swapIx;
    }
    catch (err) {
        console.error("buildSwapInstruction failed:", err);
        throw err;
    }
}
/**
 * Calculate price from Pool Keys (fetching reserves)
 */
async function getPoolPrice(connection, poolKeys) {
    try {
        // Fetch vault accounts
        const baseVaultInfo = await connection.getAccountInfo(poolKeys.baseVault);
        const quoteVaultInfo = await connection.getAccountInfo(poolKeys.quoteVault);
        if (!baseVaultInfo || !quoteVaultInfo)
            return 0;
        // Parse token amounts (assuming standard layout or just use offsets)
        // Token Account Layout: mint(32) + owner(32) + amount(8) ...
        // Amount is at offset 64
        const baseReserve = new bn_js_1.default(baseVaultInfo.data.slice(64, 72), "le");
        const quoteReserve = new bn_js_1.default(quoteVaultInfo.data.slice(64, 72), "le");
        const baseDecimals = poolKeys.baseDecimals;
        const quoteDecimals = poolKeys.quoteDecimals;
        const baseAmount = parseFloat(baseReserve.toString()) / Math.pow(10, baseDecimals);
        const quoteAmount = parseFloat(quoteReserve.toString()) / Math.pow(10, quoteDecimals);
        if (baseAmount === 0)
            return 0;
        return quoteAmount / baseAmount;
    }
    catch (err) {
        console.error("getPoolPrice failed:", err);
        return 0;
    }
}
/**
 * Calculate output amount locally using Raydium SDK
 */
async function calculateAmountOut(connection, poolKeys, amountIn, // raw amount
slippageBps, inputMint) {
    try {
        // Fetch vault reserves
        const baseVaultInfo = await connection.getAccountInfo(poolKeys.baseVault);
        const quoteVaultInfo = await connection.getAccountInfo(poolKeys.quoteVault);
        if (!baseVaultInfo || !quoteVaultInfo)
            throw new Error("Failed to fetch vault info");
        const baseReserve = new bn_js_1.default(baseVaultInfo.data.slice(64, 72), "le");
        const quoteReserve = new bn_js_1.default(quoteVaultInfo.data.slice(64, 72), "le");
        // Construct Pool Info
        const poolInfo = {
            status: new bn_js_1.default(6),
            baseDecimals: poolKeys.baseDecimals,
            quoteDecimals: poolKeys.quoteDecimals,
            lpDecimals: poolKeys.lpDecimals,
            baseReserve,
            quoteReserve,
            lpSupply: new bn_js_1.default(0),
            startTime: new bn_js_1.default(0)
        };
        const amountInBN = new bn_js_1.default(amountIn);
        const slippage = new raydium_sdk_1.Percent(slippageBps, 10000);
        let currencyIn;
        let currencyOut;
        if (inputMint.toBase58() === poolKeys.baseMint.toBase58()) {
            currencyIn = new raydium_sdk_1.Token(spl_token_1.TOKEN_PROGRAM_ID, poolKeys.baseMint, poolKeys.baseDecimals);
            currencyOut = new raydium_sdk_1.Token(spl_token_1.TOKEN_PROGRAM_ID, poolKeys.quoteMint, poolKeys.quoteDecimals);
        }
        else {
            currencyIn = new raydium_sdk_1.Token(spl_token_1.TOKEN_PROGRAM_ID, poolKeys.quoteMint, poolKeys.quoteDecimals);
            currencyOut = new raydium_sdk_1.Token(spl_token_1.TOKEN_PROGRAM_ID, poolKeys.baseMint, poolKeys.baseDecimals);
        }
        const amountInToken = new raydium_sdk_1.TokenAmount(currencyIn, amountInBN);
        const { amountOut, minAmountOut } = raydium_sdk_1.Liquidity.computeAmountOut({
            poolKeys,
            poolInfo,
            amountIn: amountInToken,
            currencyOut,
            slippage
        });
        return { minAmountOut: minAmountOut.raw, amountOut: amountOut.raw };
    }
    catch (err) {
        console.error("calculateAmountOut failed:", err);
        return { minAmountOut: new bn_js_1.default(0), amountOut: new bn_js_1.default(0) };
    }
}
/**
 * Fetch Raydium pool info from on-chain data
 */
async function getRaydiumPoolInfo(connection, poolAddress) {
    try {
        const poolPubkey = new web3_js_1.PublicKey(poolAddress);
        const poolAccount = await connection.getAccountInfo(poolPubkey);
        if (!poolAccount) {
            console.error(`Pool not found: ${poolAddress}`);
            return null;
        }
        // Raydium V4 pool data layout (simplified)
        // Note: Full parsing requires detailed knowledge of Raydium's IDL
        // For production, use Raydium SDK
        console.log(`📊 Pool account size: ${poolAccount.data.length} bytes`);
        // Placeholder: In production, parse the actual buffer
        return null;
    }
    catch (err) {
        console.error("getRaydiumPoolInfo failed:", err);
        return null;
    }
}
/**
 * Get token price from Raydium via Jupiter Price API
 * Falls back to simple ratio calculation
 */
async function getTokenPrice(mint, baseMint = "So11111111111111111111111111111111111111112" // SOL
) {
    try {
        // Try Jupiter API first
        const response = await axios_1.default.get("https://price.jup.ag/v4/price", {
            params: {
                ids: mint,
                vsToken: baseMint,
            },
            timeout: 5000,
        });
        const priceData = response.data.data[mint];
        if (priceData && priceData.price) {
            return parseFloat(priceData.price);
        }
        console.warn(`No price data from Jupiter for ${mint}`);
        return 0;
    }
    catch (err) {
        console.error("getTokenPrice failed:", err);
        return 0;
    }
}
/**
 * Simulate a swap to estimate output amount
 * Uses Jupiter SDK under the hood
 */
async function estimateSwapAmount(inputMint, outputMint, inputAmount, // in lamports or raw decimals
slippageBps = 1500) {
    try {
        const inputDecimals = 9; // Assuming SOL for now
        const outputDecimals = 6; // Most tokens
        // Convert to base units
        const inputAmountBase = inputAmount * Math.pow(10, inputDecimals);
        // Query Jupiter for quote
        const response = await axios_1.default.get("https://quote-api.jup.ag/v6/quote", {
            params: {
                inputMint,
                outputMint,
                amount: Math.floor(inputAmountBase),
                slippageBps,
            },
            timeout: 5000,
        });
        const quote = response.data;
        return {
            outputAmount: Number(quote.outAmount) / Math.pow(10, outputDecimals),
            minOutputAmount: Number(quote.outAmount) * (1 - slippageBps / 10000) /
                Math.pow(10, outputDecimals),
            priceImpact: parseFloat(quote.priceImpactPct || "0"),
        };
    }
    catch (err) {
        console.error("estimateSwapAmount failed:", err);
        return null;
    }
}
/**
 * Get a swap transaction from Jupiter API
 */
async function getSwapTransaction(userPublicKey, inputMint, outputMint, amount, // in raw units (lamports/atoms)
slippageBps = 500) {
    try {
        // 1. Get Quote
        const quoteResponse = await axios_1.default.get("https://quote-api.jup.ag/v6/quote", {
            params: {
                inputMint,
                outputMint,
                amount,
                slippageBps,
            },
        });
        const quoteData = quoteResponse.data;
        if (!quoteData) {
            throw new Error("No quote found");
        }
        // 2. Get Swap Transaction
        const swapResponse = await axios_1.default.post("https://quote-api.jup.ag/v6/swap", {
            quoteResponse: quoteData,
            userPublicKey,
            wrapAndUnwrapSol: true,
            // prioritizeFeeLamports: 10000, // We add Jito tip separately
        });
        return swapResponse.data.swapTransaction;
    }
    catch (err) {
        console.error("getSwapTransaction failed:", err);
        return null;
    }
}
/**
 * Convert lamports to SOL
 */
function lamportsToSol(lamports) {
    return lamports / 1e9;
}
/**
 * Convert SOL to lamports
 */
function solToLamports(sol) {
    return Math.floor(sol * 1e9);
}
//# sourceMappingURL=raydium.js.map