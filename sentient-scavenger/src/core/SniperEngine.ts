import {
  Connection,
  PublicKey,
  Transaction,
  Keypair,
  SystemProgram,
  TransactionInstruction,
  SYSVAR_RENT_PUBKEY,
  VersionedTransaction,
  TransactionMessage,
} from "@solana/web3.js";
import {
  createAssociatedTokenAccountIdempotentInstruction,
  getAssociatedTokenAddress,
  TOKEN_PROGRAM_ID,
  createSyncNativeInstruction,
} from "@solana/spl-token";
import { ComputeBudgetProgram } from "@solana/web3.js";
import { getCachedBlockhash } from "../services/BlockhashManager";
import { JitoExecutor } from "../services/JitoExecutor";
import { SenderExecutor } from "../services/SenderExecutor";
import {
  WSOL_MINT,
  SOL_WAGER_AMOUNT,
  SLIPPAGE_BPS,
  DRY_RUN,
  RAYDIUM_V4_PROGRAM,
  SYSTEM_PROGRAM,
  WSOL_DECIMALS,
} from "../config";
import {
  estimateSwapAmount,
  solToLamports,
  buildSwapInstruction,
  calculateAmountOut,
} from "../utils/raydium";
import * as BufferLayout from "@solana/buffer-layout";
import { LiquidityPoolKeysV4 } from "@raydium-io/raydium-sdk";

interface BuySignal {
  mint: string;
  name: string;
  description: string;
  poolKeys: LiquidityPoolKeysV4;
  twitterHandle?: string;
}

export class SniperEngine {
  private connection: Connection;
  private keypair: Keypair;
  private jitoExecutor: JitoExecutor;
  private senderExecutor: SenderExecutor;
  private lastEntryPrice: Map<string, number> = new Map();
  private poolKeysMap: Map<string, LiquidityPoolKeysV4> = new Map();

  constructor(
    connection: Connection,
    keypair: Keypair,
    jitoExecutor: JitoExecutor,
    senderExecutor: SenderExecutor
  ) {
    this.connection = connection;
    this.keypair = keypair;
    this.jitoExecutor = jitoExecutor;
    this.senderExecutor = senderExecutor;
  }

  async buy(signal: BuySignal): Promise<string | null> {
    try {
      console.log(`🎯 SNIPE SIGNAL: ${signal.name} (${signal.mint})`);
      this.poolKeysMap.set(signal.mint, signal.poolKeys);

      const amountIn = solToLamports(SOL_WAGER_AMOUNT);
      
      // Calculate slippage locally
      const { minAmountOut: minAmountOutBN } = await calculateAmountOut(
        this.connection,
        signal.poolKeys,
        amountIn,
        SLIPPAGE_BPS,
        new PublicKey(WSOL_MINT)
      );
      const minAmountOut = minAmountOutBN.toNumber();

      // 1. Setup Token Accounts
      const wsolAta = await getAssociatedTokenAddress(
        new PublicKey(WSOL_MINT),
        this.keypair.publicKey
      );
      const tokenAta = await getAssociatedTokenAddress(
        new PublicKey(signal.mint),
        this.keypair.publicKey
      );

      const instructions: TransactionInstruction[] = [];

      // Optional compute budget bump for reliability
      instructions.push(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 100_000 })
      );

      // 0. Ensure WSOL ATA exists and is funded
      const wsolPrep = await this.prepareWsolAccount(wsolAta, amountIn);
      instructions.push(...wsolPrep);

      // 2. Create Token ATA (Destination)
      instructions.push(
        createAssociatedTokenAccountIdempotentInstruction(
          this.keypair.publicKey,
          tokenAta,
          this.keypair.publicKey,
          new PublicKey(signal.mint)
        )
      );

      // 3. Build Swap Instruction
      const swapIx = await buildSwapInstruction(
        this.connection,
        signal.poolKeys,
        this.keypair.publicKey,
        wsolAta, // Input (WSOL)
        tokenAta, // Output (Token)
        amountIn,
        minAmountOut
      );
      instructions.push(swapIx);

      // 5. Build Transaction
      const blockhash = getCachedBlockhash();
      if (!blockhash) throw new Error("No blockhash");

      const messageV0 = new TransactionMessage({
        payerKey: this.keypair.publicKey,
        recentBlockhash: blockhash.blockhash,
        instructions,
      }).compileToV0Message();

      const transaction = new VersionedTransaction(messageV0);
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
            const entryPrice = SOL_WAGER_AMOUNT / tokensReceived;

            this.lastEntryPrice.set(signal.mint, entryPrice);
            console.log(
              `� Entry Price: ${entryPrice.toFixed(9)} SOL (Est. Tokens: ${tokensReceived})`
            );

            return bundleId;
          } else {
            console.warn(
              `⚠️ Bundle NOT confirmed for ${signal.mint}: ${status.error || "unknown"}`
            );
            return null; // Do not record position
          }
        } catch (err) {
          console.error(`Bundle status wait failed for ${bundleId}:`, err);
          return null;
        }
      } else if (DRY_RUN) {
        this.lastEntryPrice.set(signal.mint, 1.0);
        console.log(`✅ (DRY RUN) Buy would execute`);
        return "DRY_RUN_BUNDLE_ID";
      }

      return null;
    } catch (err) {
      console.error("SniperEngine.buy() failed:", err);
      return null;
    }
  }

  async sell(mint: string, amount: number, isEmergency: boolean = false): Promise<string | null> {
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
         const { minAmountOut: minOutBN } = await calculateAmountOut(
            this.connection,
            poolKeys,
            amountIn,
            SLIPPAGE_BPS,
            new PublicKey(mint)
         );
         minAmountOut = minOutBN.toNumber();
      }

      // 1. Setup Token Accounts
      const wsolAta = await getAssociatedTokenAddress(
        new PublicKey(WSOL_MINT),
        this.keypair.publicKey
      );
      const tokenAta = await getAssociatedTokenAddress(
        new PublicKey(mint),
        this.keypair.publicKey
      );

      const instructions: TransactionInstruction[] = [];

      // 2. Build Swap Instruction (Token -> SOL)
      const swapIx = await buildSwapInstruction(
        this.connection,
        poolKeys,
        this.keypair.publicKey,
        tokenAta, // Input (Token)
        wsolAta, // Output (WSOL)
        amountIn,
        minAmountOut
      );
      instructions.push(swapIx);

      // 3. Close WSOL account to unwrap SOL (optional, or keep as wSOL)
      // For now, we keep as wSOL to save gas/time, or unwrap if needed.
      // Let's unwrap to realize profit in SOL.
      // instructions.push(createCloseAccountInstruction(wsolAta, this.keypair.publicKey, this.keypair.publicKey));

      // 4. Build Transaction
      const blockhash = getCachedBlockhash();
      if (!blockhash) throw new Error("No blockhash");

      const messageV0 = new TransactionMessage({
        payerKey: this.keypair.publicKey,
        recentBlockhash: blockhash.blockhash,
        instructions,
      }).compileToV0Message();

      const transaction = new VersionedTransaction(messageV0);
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
            } else {
              console.warn(
                `⚠️ Sell bundle NOT confirmed for ${mint}: ${status.error || "unknown"}`
              );
            }
          })
          .catch((err) => {
            console.error(`Sell bundle status wait failed for ${bundleId}:`, err);
          });
        return bundleId;
      } else if (DRY_RUN) {
        console.log(`✅ (DRY RUN) Sell would execute`);
        return "DRY_RUN_SELL_ID";
      }

      return null;
    } catch (err) {
      console.error("SniperEngine.sell() failed:", err);
      return null;
    }
  }

  /**
   * Get entry price for position tracking
   */
  getEntryPrice(mint: string): number {
    return this.lastEntryPrice.get(mint) || 0;
  }

  /**
   * Register pool keys manually (e.g. from persistence)
   */
  registerPoolKeys(mint: string, keys: LiquidityPoolKeysV4) {
    this.poolKeysMap.set(mint, keys);
  }

  private async prepareWsolAccount(
    wsolAta: PublicKey,
    requiredLamports: number
  ): Promise<TransactionInstruction[]> {
    const instructions: TransactionInstruction[] = [];
    const needed = BigInt(requiredLamports);

    let current = BigInt(0);
    let accountExists = false;

    try {
      const balance = await this.connection.getTokenAccountBalance(wsolAta);
      if (balance?.value?.amount) {
        current = BigInt(balance.value.amount);
        accountExists = true;
      }
    } catch {
      accountExists = false;
    }

    if (!accountExists) {
      instructions.push(
        createAssociatedTokenAccountIdempotentInstruction(
          this.keypair.publicKey,
          wsolAta,
          this.keypair.publicKey,
          new PublicKey(WSOL_MINT)
        )
      );
    }

    if (current < needed) {
      const diff = Number(needed - current);
      instructions.push(
        SystemProgram.transfer({
          fromPubkey: this.keypair.publicKey,
          toPubkey: wsolAta,
          lamports: diff,
        }),
        createSyncNativeInstruction(wsolAta)
      );
    }

    return instructions;
  }
}
