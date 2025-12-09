export interface WhitelistEntry {
  mint: string;
  decimals: number;
  expiresAt: number;
}

/**
 * Simple TTL whitelist with atomic consume semantics.
 */
export class Whitelist {
  private entries: Map<string, WhitelistEntry> = new Map();
  private ttlMs: number;

  constructor(ttlMs: number) {
    this.ttlMs = ttlMs;
  }

  upsert(entry: Omit<WhitelistEntry, "expiresAt">): void {
    this.entries.set(entry.mint, {
      ...entry,
      expiresAt: Date.now() + this.ttlMs,
    });
  }

  /**
   * Atomically consume (read + delete) a whitelist entry.
   */
  consume(mint: string): WhitelistEntry | undefined {
    this.prune();
    const entry = this.entries.get(mint);
    if (entry) {
      this.entries.delete(mint);
    }
    return entry;
  }

  prune(): void {
    const now = Date.now();
    for (const [mint, entry] of this.entries.entries()) {
      if (entry.expiresAt <= now) {
        this.entries.delete(mint);
      }
    }
  }

  size(): number {
    this.prune();
    return this.entries.size;
  }

  /**
   * Check existence without consuming (for early filters).
   */
  has(mint: string): boolean {
    this.prune();
    return this.entries.has(mint);
  }
}
