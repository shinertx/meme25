import { Connection } from "@solana/web3.js";
export declare function initializeBlockhashManager(connection: Connection): Promise<void>;
export declare function getCachedBlockhash(): {
    blockhash: string;
    lastValidBlockHeight: number;
} | null;
//# sourceMappingURL=BlockhashManager.d.ts.map