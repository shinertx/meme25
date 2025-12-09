import { Connection } from "@solana/web3.js";
import { Whitelist } from "../services/Whitelist";
/**
 * Producer: watches Pump.fun, prefilters, runs heavy checks, and populates the whitelist.
 * This keeps heavy work off the Raydium hot path.
 */
export declare class PumpPreCog {
    private connection;
    private heliusRpcUrl;
    private whitelist;
    constructor(connection: Connection, heliusRpcUrl: string, whitelist: Whitelist);
    private logCount;
    getVelocity(): number;
    start(): Promise<void>;
    private isNearComplete;
    private extractMintFromLogs;
    private checkSocials;
    private checkCabal;
    private checkMintAuthorities;
}
//# sourceMappingURL=PumpPreCog.d.ts.map