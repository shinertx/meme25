"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.Whitelist = void 0;
/**
 * Simple TTL whitelist with atomic consume semantics.
 */
class Whitelist {
    constructor(ttlMs) {
        this.entries = new Map();
        this.ttlMs = ttlMs;
    }
    upsert(entry) {
        this.entries.set(entry.mint, {
            ...entry,
            expiresAt: Date.now() + this.ttlMs,
        });
    }
    /**
     * Atomically consume (read + delete) a whitelist entry.
     */
    consume(mint) {
        this.prune();
        const entry = this.entries.get(mint);
        if (entry) {
            this.entries.delete(mint);
        }
        return entry;
    }
    prune() {
        const now = Date.now();
        for (const [mint, entry] of this.entries.entries()) {
            if (entry.expiresAt <= now) {
                this.entries.delete(mint);
            }
        }
    }
    size() {
        this.prune();
        return this.entries.size;
    }
    /**
     * Check existence without consuming (for early filters).
     */
    has(mint) {
        this.prune();
        return this.entries.has(mint);
    }
}
exports.Whitelist = Whitelist;
//# sourceMappingURL=Whitelist.js.map