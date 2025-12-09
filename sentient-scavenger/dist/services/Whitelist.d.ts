export interface WhitelistEntry {
    mint: string;
    decimals: number;
    expiresAt: number;
}
/**
 * Simple TTL whitelist with atomic consume semantics.
 */
export declare class Whitelist {
    private entries;
    private ttlMs;
    constructor(ttlMs: number);
    upsert(entry: Omit<WhitelistEntry, "expiresAt">): void;
    /**
     * Atomically consume (read + delete) a whitelist entry.
     */
    consume(mint: string): WhitelistEntry | undefined;
    prune(): void;
    size(): number;
    /**
     * Check existence without consuming (for early filters).
     */
    has(mint: string): boolean;
}
//# sourceMappingURL=Whitelist.d.ts.map