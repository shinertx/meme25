import { 
  Connection, 
  PublicKey, 
  TransactionInstruction,
  SystemProgram 
} from "@solana/web3.js";
import { 
  Liquidity, 
  LiquidityPoolKeys, 
  LiquidityPoolKeysV4,
  Percent, 
  Token, 
  TokenAmount, 
  Currency,
  SOL,
  MAINNET_PROGRAM_ID,
  jsonInfo2PoolKeys,
  LiquidityPoolJsonInfo,
  TxVersion
} from "@raydium-io/raydium-sdk";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { RAYDIUM_V4_PROGRAM } from "../config";
import axios from "axios";
import BN from "bn.js";

/**
 * Build a Raydium Swap Instruction locally (No API calls)
 */
export async function buildSwapInstruction(
  connection: Connection,
  poolKeys: LiquidityPoolKeysV4,
  userPublicKey: PublicKey,
  inputTokenAccount: PublicKey,
  outputTokenAccount: PublicKey,
  amountIn: number, // raw amount (lamports/atoms)
  minAmountOut: number, // raw amount
  fixedSide: "in" | "out" = "in"
): Promise<TransactionInstruction> {
  try {
    const amountInBN = new BN(amountIn);
    const minAmountOutBN = new BN(minAmountOut);

    const baseToken = new Token(TOKEN_PROGRAM_ID, poolKeys.baseMint, poolKeys.baseDecimals);
    const quoteToken = new Token(TOKEN_PROGRAM_ID, poolKeys.quoteMint, poolKeys.quoteDecimals);

    const amountInToken = new TokenAmount(baseToken, amountInBN);
    const amountOutToken = new TokenAmount(quoteToken, minAmountOutBN);

    const { innerTransactions } = await Liquidity.makeSwapInstructionSimple({
        connection,
        poolKeys,
        userKeys: {
            tokenAccounts: [
                // We don't need real balances for instruction generation, but we need correct owners
                { pubkey: inputTokenAccount, programId: TOKEN_PROGRAM_ID, accountInfo: { mint: poolKeys.baseMint, owner: userPublicKey, amount: new BN(0), delegateOption: 0, delegate: PublicKey.default, state: 1, isNativeOption: 0, isNative: false, delegatedAmount: new BN(0), closeAuthorityOption: 0, closeAuthority: PublicKey.default } as any },
                { pubkey: outputTokenAccount, programId: TOKEN_PROGRAM_ID, accountInfo: { mint: poolKeys.quoteMint, owner: userPublicKey, amount: new BN(0), delegateOption: 0, delegate: PublicKey.default, state: 1, isNativeOption: 0, isNative: false, delegatedAmount: new BN(0), closeAuthorityOption: 0, closeAuthority: PublicKey.default } as any }
            ],
            owner: userPublicKey,
        },
        amountIn: amountInToken,
        amountOut: amountOutToken,
        fixedSide: "in",
        makeTxVersion: TxVersion.V0,
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
    const swapIx = instructions.find(ix => ix.programId.equals(new PublicKey(RAYDIUM_V4_PROGRAM)));
    
    if (!swapIx) {
        // Fallback: It might be the last instruction if SDK structure changes
        return instructions[instructions.length - 1];
    }
    
    return swapIx;

  } catch (err) {
    console.error("buildSwapInstruction failed:", err);
    throw err;
  }
}

/**
 * Calculate price from Pool Keys (fetching reserves)
 */
export async function getPoolPrice(
  connection: Connection,
  poolKeys: LiquidityPoolKeysV4
): Promise<number> {
  try {
    // Fetch vault accounts
    const baseVaultInfo = await connection.getAccountInfo(poolKeys.baseVault);
    const quoteVaultInfo = await connection.getAccountInfo(poolKeys.quoteVault);

    if (!baseVaultInfo || !quoteVaultInfo) return 0;

    // Parse token amounts (assuming standard layout or just use offsets)
    // Token Account Layout: mint(32) + owner(32) + amount(8) ...
    // Amount is at offset 64
    const baseReserve = new BN(baseVaultInfo.data.slice(64, 72), "le");
    const quoteReserve = new BN(quoteVaultInfo.data.slice(64, 72), "le");

    const baseDecimals = poolKeys.baseDecimals;
    const quoteDecimals = poolKeys.quoteDecimals;

    const baseAmount = parseFloat(baseReserve.toString()) / Math.pow(10, baseDecimals);
    const quoteAmount = parseFloat(quoteReserve.toString()) / Math.pow(10, quoteDecimals);

    if (baseAmount === 0) return 0;

    return quoteAmount / baseAmount;
  } catch (err) {
    console.error("getPoolPrice failed:", err);
    return 0;
  }
}

/**
 * Calculate output amount locally using Raydium SDK
 */
export async function calculateAmountOut(
  connection: Connection,
  poolKeys: LiquidityPoolKeysV4,
  amountIn: number, // raw amount
  slippageBps: number,
  inputMint: PublicKey
): Promise<{ minAmountOut: BN, amountOut: BN }> {
  try {
    // Fetch vault reserves
    const baseVaultInfo = await connection.getAccountInfo(poolKeys.baseVault);
    const quoteVaultInfo = await connection.getAccountInfo(poolKeys.quoteVault);
    
    if (!baseVaultInfo || !quoteVaultInfo) throw new Error("Failed to fetch vault info");

    const baseReserve = new BN(baseVaultInfo.data.slice(64, 72), "le");
    const quoteReserve = new BN(quoteVaultInfo.data.slice(64, 72), "le");

    // Construct Pool Info
    const poolInfo = {
      status: new BN(6),
      baseDecimals: poolKeys.baseDecimals,
      quoteDecimals: poolKeys.quoteDecimals,
      lpDecimals: poolKeys.lpDecimals,
      baseReserve,
      quoteReserve,
      lpSupply: new BN(0),
      startTime: new BN(0)
    };

    const amountInBN = new BN(amountIn);
    const slippage = new Percent(slippageBps, 10000);
    
    let currencyIn: Token;
    let currencyOut: Token;

    if (inputMint.toBase58() === poolKeys.baseMint.toBase58()) {
        currencyIn = new Token(TOKEN_PROGRAM_ID, poolKeys.baseMint, poolKeys.baseDecimals);
        currencyOut = new Token(TOKEN_PROGRAM_ID, poolKeys.quoteMint, poolKeys.quoteDecimals);
    } else {
        currencyIn = new Token(TOKEN_PROGRAM_ID, poolKeys.quoteMint, poolKeys.quoteDecimals);
        currencyOut = new Token(TOKEN_PROGRAM_ID, poolKeys.baseMint, poolKeys.baseDecimals);
    }

    const amountInToken = new TokenAmount(currencyIn, amountInBN);

    const { amountOut, minAmountOut } = Liquidity.computeAmountOut({
        poolKeys,
        poolInfo,
        amountIn: amountInToken,
        currencyOut,
        slippage
    });

    return { minAmountOut: minAmountOut.raw, amountOut: amountOut.raw };
  } catch (err) {
    console.error("calculateAmountOut failed:", err);
    return { minAmountOut: new BN(0), amountOut: new BN(0) };
  }
}

/**
 * Fetch Raydium pool info from on-chain data
 */
export async function getRaydiumPoolInfo(
  connection: Connection,
  poolAddress: string
): Promise<{
  tokenAMint: string;
  tokenBMint: string;
  tokenADecimals: number;
  tokenBDecimals: number;
  reserveA: number;
  reserveB: number;
} | null> {
  try {
    const poolPubkey = new PublicKey(poolAddress);
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
  } catch (err) {
    console.error("getRaydiumPoolInfo failed:", err);
    return null;
  }
}

/**
 * Get token price from Raydium via Jupiter Price API
 * Falls back to simple ratio calculation
 */
export async function getTokenPrice(
  mint: string,
  baseMint: string = "So11111111111111111111111111111111111111112" // SOL
): Promise<number> {
  try {
    // Try Jupiter API first
    const response = await axios.get("https://price.jup.ag/v4/price", {
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
  } catch (err) {
    console.error("getTokenPrice failed:", err);
    return 0;
  }
}

/**
 * Simulate a swap to estimate output amount
 * Uses Jupiter SDK under the hood
 */
export async function estimateSwapAmount(
  inputMint: string,
  outputMint: string,
  inputAmount: number, // in lamports or raw decimals
  slippageBps: number = 1500
): Promise<{
  outputAmount: number;
  minOutputAmount: number;
  priceImpact: number;
} | null> {
  try {
    const inputDecimals = 9; // Assuming SOL for now
    const outputDecimals = 6; // Most tokens

    // Convert to base units
    const inputAmountBase = inputAmount * Math.pow(10, inputDecimals);

    // Query Jupiter for quote
    const response = await axios.get("https://quote-api.jup.ag/v6/quote", {
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
      minOutputAmount:
        Number(quote.outAmount) * (1 - slippageBps / 10000) /
        Math.pow(10, outputDecimals),
      priceImpact: parseFloat(quote.priceImpactPct || "0"),
    };
  } catch (err) {
    console.error("estimateSwapAmount failed:", err);
    return null;
  }
}

/**
 * Get a swap transaction from Jupiter API
 */
export async function getSwapTransaction(
  userPublicKey: string,
  inputMint: string,
  outputMint: string,
  amount: number, // in raw units (lamports/atoms)
  slippageBps: number = 500
): Promise<string | null> {
  try {
    // 1. Get Quote
    const quoteResponse = await axios.get("https://quote-api.jup.ag/v6/quote", {
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
    const swapResponse = await axios.post("https://quote-api.jup.ag/v6/swap", {
      quoteResponse: quoteData,
      userPublicKey,
      wrapAndUnwrapSol: true,
      // prioritizeFeeLamports: 10000, // We add Jito tip separately
    });

    return swapResponse.data.swapTransaction;
  } catch (err) {
    console.error("getSwapTransaction failed:", err);
    return null;
  }
}

/**
 * Convert lamports to SOL
 */
export function lamportsToSol(lamports: number): number {
  return lamports / 1e9;
}

/**
 * Convert SOL to lamports
 */
export function solToLamports(sol: number): number {
  return Math.floor(sol * 1e9);
}
