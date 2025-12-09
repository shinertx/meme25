import { Connection, Keypair } from "@solana/web3.js";
import { BLOCKHASH_POLL_INTERVAL } from "../config";

let latestBlockHash: {
  blockhash: string;
  lastValidBlockHeight: number;
  timestamp: number;
} | null = null;

export async function initializeBlockhashManager(
  connection: Connection
): Promise<void> {
  // Initial fetch
  await updateBlockhash(connection);

  // Start polling loop with backoff
  pollBlockhash(connection);
}

async function pollBlockhash(connection: Connection) {
  try {
    await updateBlockhash(connection);
    // Success: wait standard interval
    setTimeout(() => pollBlockhash(connection), BLOCKHASH_POLL_INTERVAL);
  } catch (err) {
    // Error: wait longer (backoff)
    // console.error("Blockhash poll failed, backing off...");
    setTimeout(() => pollBlockhash(connection), BLOCKHASH_POLL_INTERVAL * 5);
  }
}

async function updateBlockhash(connection: Connection): Promise<void> {
  try {
    const bh = await connection.getLatestBlockhash("processed");
    latestBlockHash = {
      blockhash: bh.blockhash,
      lastValidBlockHeight: bh.lastValidBlockHeight,
      timestamp: Date.now(),
    };
  } catch (err) {
    // console.error("Blockhash update failed:", err);
  }
}

export function getCachedBlockhash(): {
  blockhash: string;
  lastValidBlockHeight: number;
} | null {
  if (!latestBlockHash) return null;

  // Return cached if less than 2 seconds old
  if (Date.now() - latestBlockHash.timestamp < 2000) {
    return {
      blockhash: latestBlockHash.blockhash,
      lastValidBlockHeight: latestBlockHash.lastValidBlockHeight,
    };
  }

  // Stale, but return anyway (it will be refreshed on next poll)
  return {
    blockhash: latestBlockHash.blockhash,
    lastValidBlockHeight: latestBlockHash.lastValidBlockHeight,
  };
}
