import { Connection, Keypair } from "@solana/web3.js";
export declare class Janitor {
    private connection;
    private keypair;
    constructor(connection: Connection, keypair: Keypair);
    cleanupEmptyTokenAccounts(): Promise<void>;
    startMaintenanceLoop(intervalMs: number): Promise<void>;
}
//# sourceMappingURL=Janitor.d.ts.map