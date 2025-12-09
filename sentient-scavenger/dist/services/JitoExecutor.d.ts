import { Connection, Keypair, Transaction, VersionedTransaction } from "@solana/web3.js";
interface BundleStatus {
    bundleId: string;
    confirmed: boolean;
    slot?: number;
    error?: string;
}
export declare class JitoExecutor {
    private connection;
    private keypair;
    private bundleStatuses;
    private bundleWaiters;
    constructor(connection: Connection, keypair: Keypair);
    executeAndConfirm(transaction: Transaction | VersionedTransaction, signTransaction?: boolean): Promise<string | null>;
    /**
     * Poll Jito for bundle confirmation status
     * This runs async in the background
     */
    private pollBundleConfirmation;
    /**
     * Get bundle status (if polling already completed)
     */
    getBundleStatus(bundleId: string): BundleStatus | undefined;
    /**
     * Await bundle confirmation result (resolves when poller finishes)
     */
    waitForBundleStatus(bundleId: string, timeoutMs?: number): Promise<BundleStatus>;
    private resolveBundleWaiters;
    /**
     * Calculate dynamic Jito tip
     * Spec: min(0.005 SOL, 1% of wagered amount), floor at 0.0005 SOL
     * Potential profit assumed to be 100% of wager (2x)
     */
    private calculateDynamicTip;
    /**
     * Retry failed bundle (fire again)
     */
    retryBundle(transaction: Transaction): Promise<string | null>;
}
export {};
//# sourceMappingURL=JitoExecutor.d.ts.map