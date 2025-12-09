import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  VersionedTransaction,
} from "@solana/web3.js";
import { getCachedBlockhash } from "./BlockhashManager";
import axios from "axios";
import bs58 from "bs58";
import {
  JITO_BLOCK_ENGINE_URL,
  JITO_TIP_ACCOUNTS,
  DRY_RUN,
  BUNDLE_CONFIRMATION_POLL,
  BUNDLE_CONFIRMATION_TIMEOUT,
  SOL_WAGER_AMOUNT,
} from "../config";

interface BundleStatus {
  bundleId: string;
  confirmed: boolean;
  slot?: number;
  error?: string;
}

export class JitoExecutor {
  private connection: Connection;
  private keypair: Keypair;
  private bundleStatuses: Map<string, BundleStatus> = new Map();
  private bundleWaiters: Map<string, Array<(status: BundleStatus) => void>> =
    new Map();

  constructor(connection: Connection, keypair: Keypair) {
    this.connection = connection;
    this.keypair = keypair;
  }

  async executeAndConfirm(
    transaction: Transaction | VersionedTransaction,
    signTransaction: boolean = true
  ): Promise<string | null> {
    try {
      const blockhash = getCachedBlockhash();
      if (!blockhash) {
        throw new Error("No cached blockhash available");
      }

      // Sign the main transaction if needed
      if (signTransaction) {
        if (transaction instanceof VersionedTransaction) {
          transaction.sign([this.keypair]);
        } else {
          transaction.recentBlockhash = blockhash.blockhash;
          transaction.sign(this.keypair);
        }
      }

      // Create separate Tip Transaction
      const tipAmount = this.calculateDynamicTip();
      const tipAccount =
        JITO_TIP_ACCOUNTS[
          Math.floor(Math.random() * JITO_TIP_ACCOUNTS.length)
        ];
      
      // console.log(`DEBUG: Using Tip Account: ${tipAccount}`);

      const tipTx = new Transaction();
      tipTx.add(
        SystemProgram.transfer({
          fromPubkey: this.keypair.publicKey,
          toPubkey: new PublicKey(tipAccount),
          lamports: Math.floor(tipAmount * 1e9),
        })
      );
      tipTx.recentBlockhash = blockhash.blockhash;
      tipTx.feePayer = this.keypair.publicKey;
      tipTx.sign(this.keypair);

      if (DRY_RUN) {
        console.log("🔬 DRY RUN: Would send bundle to Jito");
        console.log("  Tip Account: " + tipAccount);
        console.log("  Tip Amount: " + tipAmount.toFixed(4) + " SOL");
        return "DRY_RUN_BUNDLE_" + Math.random().toString(36).substring(7);
      }

      // Serialize both transactions
      const encodedTx = bs58.encode(
        transaction instanceof VersionedTransaction
          ? transaction.serialize()
          : transaction.serialize()
      );
      const encodedTipTx = bs58.encode(tipTx.serialize());

      // Send bundle [MainTx, TipTx]
      console.log("📤 Submitting bundle to Jito...");
      const response = await axios.post(JITO_BLOCK_ENGINE_URL, {
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
    } catch (err) {
      console.error("JitoExecutor.executeAndConfirm failed:", err);
      return null;
    }
  }

  /**
   * Poll Jito for bundle confirmation status
   * This runs async in the background
   */
  private async pollBundleConfirmation(bundleId: string): Promise<void> {
    const startTime = Date.now();
    let attempts = 0;
    const maxAttempts = Math.floor(
      BUNDLE_CONFIRMATION_TIMEOUT / BUNDLE_CONFIRMATION_POLL
    );

    console.log(
      `⏳ Polling bundle status (timeout: ${BUNDLE_CONFIRMATION_TIMEOUT / 1000}s)...`
    );

    while (attempts < maxAttempts) {
      try {
        const response = await axios.post(JITO_BLOCK_ENGINE_URL, {
          jsonrpc: "2.0",
          id: 1,
          method: "getBundleStatuses",
          params: [[bundleId]],
        });

        if (response.data.result && response.data.result.value && response.data.result.value.length > 0) {
          const status = response.data.result.value[0];

          if (status && (status.confirmation_status === "confirmed" || status.confirmation_status === "finalized")) {
            console.log(
              `✅ Bundle CONFIRMED in slot ${status.slot}: ${bundleId}`
            );
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
        await new Promise((resolve) =>
          setTimeout(resolve, BUNDLE_CONFIRMATION_POLL)
        );
      } catch (err) {
        console.warn(`Bundle status check failed (attempt ${attempts}):`, err);
        attempts++;
        await new Promise((resolve) =>
          setTimeout(resolve, BUNDLE_CONFIRMATION_POLL * 2)
        ); // Back off on error
      }
    }

    // Timeout reached
    console.error(
      `❌ Bundle confirmation timeout: ${bundleId} (waited ${BUNDLE_CONFIRMATION_TIMEOUT / 1000}s)`
    );
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
  getBundleStatus(bundleId: string): BundleStatus | undefined {
    return this.bundleStatuses.get(bundleId);
  }

  /**
   * Await bundle confirmation result (resolves when poller finishes)
   */
  async waitForBundleStatus(
    bundleId: string,
    timeoutMs: number = BUNDLE_CONFIRMATION_TIMEOUT + 5000
  ): Promise<BundleStatus> {
    // Immediate success for Dry Run
    if (bundleId.startsWith("DRY_RUN_BUNDLE")) {
      return {
        bundleId,
        confirmed: true,
        slot: 0,
      };
    }

    const existing = this.bundleStatuses.get(bundleId);
    if (existing) return existing;

    return new Promise<BundleStatus>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.bundleWaiters.delete(bundleId);
        reject(
          new Error(
            `Bundle status timeout for ${bundleId} after ${timeoutMs}ms`
          )
        );
      }, timeoutMs);

      const handler = (status: BundleStatus) => {
        clearTimeout(timer);
        resolve(status);
      };

      const waiters = this.bundleWaiters.get(bundleId) || [];
      waiters.push(handler);
      this.bundleWaiters.set(bundleId, waiters);
    });
  }

  private resolveBundleWaiters(bundleId: string): void {
    const status = this.bundleStatuses.get(bundleId);
    const waiters = this.bundleWaiters.get(bundleId);
    if (!status || !waiters) return;
    waiters.forEach((cb) => cb(status));
    this.bundleWaiters.delete(bundleId);
  }

  /**
   * Calculate dynamic Jito tip
   * Spec: min(0.005 SOL, 1% of wagered amount), floor at 0.0005 SOL
   * Potential profit assumed to be 100% of wager (2x)
   */
  private calculateDynamicTip(): number {
    const profitEstimate = SOL_WAGER_AMOUNT; // Assume 100% gain
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
  async retryBundle(transaction: Transaction): Promise<string | null> {
    console.log("🔄 Retrying bundle with exponential backoff...");
    // TODO: Implement with exponential backoff
    return this.executeAndConfirm(transaction);
  }
}
