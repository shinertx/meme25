import { Connection, PublicKey } from "@solana/web3.js";
import { SniperEngine } from "./SniperEngine";
import { LiquidityPoolKeysV4 } from "@raydium-io/raydium-sdk";
interface TradePosition {
    mint: string;
    entryPrice: number;
    entryTime: number;
    amount: number;
    aiScore: number;
    aiAnalysis: string;
    status: "open" | "closed";
    poolKeys?: LiquidityPoolKeysV4;
}
export declare class SentientBrain {
    private connection;
    private openaiClient;
    private sniperEngine;
    private walletPublicKey;
    private activePositions;
    private monitoringIntervals;
    constructor(connection: Connection, openaiApiKey: string, sniperEngine: SniperEngine, walletPublicKey: PublicKey);
    /**
     * Load state from disk
     */
    loadState(): Promise<void>;
    /**
     * Save state to disk
     */
    private saveState;
    analyzeToken(mint: string, tokenData: any): Promise<number>;
    recordPosition(mint: string, aiScore: number, analysis: string, poolKeys?: LiquidityPoolKeysV4): Promise<void>;
    private monitorPosition;
    private getPriceFromRpc;
    sell(mint: string, reason?: string, isEmergency?: boolean, overrideAmount?: number): Promise<boolean>;
    /**
     * Get all active positions
     */
    getActivePositions(): TradePosition[];
    /**
     * Close all positions (emergency exit)
     */
    closeAll(): Promise<void>;
    private fetchTokenBalanceWithRetry;
    private bigIntToSafeNumber;
}
export {};
//# sourceMappingURL=SentientBrain.d.ts.map