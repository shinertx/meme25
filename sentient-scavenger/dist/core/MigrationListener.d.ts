import { Connection } from "@solana/web3.js";
import { SniperEngine } from "./SniperEngine";
import { SentientBrain } from "./SentientBrain";
import { Whitelist } from "../services/Whitelist";
export declare class MigrationListener {
    private connection;
    private sniperEngine;
    private sentientBrain;
    private heliusRpcUrl;
    private queue;
    private isProcessing;
    private whitelist?;
    lastLogAt: number;
    constructor(connection: Connection, sniperEngine: SniperEngine, sentientBrain: SentientBrain, heliusRpcUrl: string, whitelist?: Whitelist);
    private parsePoolKeysFromTx;
    private extractMintFromLogs;
    private logCount;
    getVelocity(): number;
    startListening(): Promise<void>;
    private processQueue;
    private checkMetadata;
    private checkCabal;
    private processMigrationLog;
    private fetchTokenMetadata;
}
//# sourceMappingURL=MigrationListener.d.ts.map