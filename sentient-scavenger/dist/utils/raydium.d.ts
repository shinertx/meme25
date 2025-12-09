import { Connection, PublicKey, TransactionInstruction } from "@solana/web3.js";
import { LiquidityPoolKeysV4 } from "@raydium-io/raydium-sdk";
import BN from "bn.js";
/**
 * Build a Raydium Swap Instruction locally (No API calls)
 */
export declare function buildSwapInstruction(connection: Connection, poolKeys: LiquidityPoolKeysV4, userPublicKey: PublicKey, inputTokenAccount: PublicKey, outputTokenAccount: PublicKey, amountIn: number, // raw amount (lamports/atoms)
minAmountOut: number, // raw amount
fixedSide?: "in" | "out"): Promise<TransactionInstruction>;
/**
 * Calculate price from Pool Keys (fetching reserves)
 */
export declare function getPoolPrice(connection: Connection, poolKeys: LiquidityPoolKeysV4): Promise<number>;
/**
 * Calculate output amount locally using Raydium SDK
 */
export declare function calculateAmountOut(connection: Connection, poolKeys: LiquidityPoolKeysV4, amountIn: number, // raw amount
slippageBps: number, inputMint: PublicKey): Promise<{
    minAmountOut: BN;
    amountOut: BN;
}>;
/**
 * Fetch Raydium pool info from on-chain data
 */
export declare function getRaydiumPoolInfo(connection: Connection, poolAddress: string): Promise<{
    tokenAMint: string;
    tokenBMint: string;
    tokenADecimals: number;
    tokenBDecimals: number;
    reserveA: number;
    reserveB: number;
} | null>;
/**
 * Get token price from Raydium via Jupiter Price API
 * Falls back to simple ratio calculation
 */
export declare function getTokenPrice(mint: string, baseMint?: string): Promise<number>;
/**
 * Simulate a swap to estimate output amount
 * Uses Jupiter SDK under the hood
 */
export declare function estimateSwapAmount(inputMint: string, outputMint: string, inputAmount: number, // in lamports or raw decimals
slippageBps?: number): Promise<{
    outputAmount: number;
    minOutputAmount: number;
    priceImpact: number;
} | null>;
/**
 * Get a swap transaction from Jupiter API
 */
export declare function getSwapTransaction(userPublicKey: string, inputMint: string, outputMint: string, amount: number, // in raw units (lamports/atoms)
slippageBps?: number): Promise<string | null>;
/**
 * Convert lamports to SOL
 */
export declare function lamportsToSol(lamports: number): number;
/**
 * Convert SOL to lamports
 */
export declare function solToLamports(sol: number): number;
//# sourceMappingURL=raydium.d.ts.map