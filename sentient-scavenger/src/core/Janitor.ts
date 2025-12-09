import { Connection, PublicKey, Keypair, SystemProgram, Transaction, sendAndConfirmTransaction } from "@solana/web3.js";
import {
  getAssociatedTokenAddress,
  createCloseAccountInstruction,
} from "@solana/spl-token";
import { DRY_RUN, TOKEN_PROGRAM } from "../config";

export class Janitor {
  private connection: Connection;
  private keypair: Keypair;

  constructor(connection: Connection, keypair: Keypair) {
    this.connection = connection;
    this.keypair = keypair;
  }

  async cleanupEmptyTokenAccounts(): Promise<void> {
    try {
      // console.log("🧹 Janitor: Scanning for empty token accounts..."); // Silenced to reduce noise

      const payer = this.keypair.publicKey;
      const accounts = await this.connection.getParsedTokenAccountsByOwner(
        payer,
        { programId: new PublicKey(TOKEN_PROGRAM) }
      );

      let recovered = 0;

      for (const { pubkey, account } of accounts.value) {
        const parsed = account.data as any;
        const amount = parsed.parsed?.info?.tokenAmount?.uiAmount || 0;

        if (amount === 0) {
          console.log(`  └─ Closing empty account: ${pubkey.toBase58()}`);

          const closeIx = createCloseAccountInstruction(pubkey, payer, payer);

          if (DRY_RUN) {
            console.log(`     (DRY RUN) Would reclaim ~0.002 SOL`);
            recovered++;
          } else {
            try {
              const tx = new Transaction().add(closeIx);
              await sendAndConfirmTransaction(this.connection, tx, [this.keypair]);
              console.log(`     ✅ Reclaimed rent from ${pubkey.toBase58()}`);
              recovered++;
            } catch (e) {
              console.error(`     ❌ Failed to close ${pubkey.toBase58()}:`, e);
            }
          }
        }
      }

      if (recovered > 0) {
        console.log(
          `✅ Janitor recovered rent from ${recovered} empty accounts`
        );
      } else {
        // console.log(`✅ Janitor: No empty accounts to cleanup`); // Silenced
      }
    } catch (err: any) {
      if (err.message?.includes("fetch failed") || err.code === "ETIMEDOUT") {
        console.warn("⚠️ Janitor network timeout (skipping this cycle)");
      } else {
        console.error("Janitor failed:", err);
      }
    }
  }

  async startMaintenanceLoop(intervalMs: number): Promise<void> {
    console.log(`🔄 Janitor loop started (interval: ${intervalMs}ms)`);
    setInterval(() => {
      this.cleanupEmptyTokenAccounts().catch((err) => {
        console.error("Janitor loop error:", err);
      });
    }, intervalMs);
  }
}
