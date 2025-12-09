import { Connection, Keypair } from "@solana/web3.js";
import { JitoExecutor } from "../services/JitoExecutor";
import { SenderExecutor } from "../services/SenderExecutor";
import { LiquidityPoolKeysV4 } from "@raydium-io/raydium-sdk";
interface BuySignal {
    mint: string;
    name: string;
    description: string;
    poolKeys: LiquidityPoolKeysV4;
    twitterHandle?: string;
}
export declare class SniperEngine {
    private connection;
    private keypair;
    private jitoExecutor;
    private senderExecutor;
    private lastEntryPrice;
    private poolKeysMap;
    constructor(connection: Connection, keypair: Keypair, jitoExecutor: JitoExecutor, senderExecutor: SenderExecutor);
    buy(signal: BuySignal): Promise<string | null>;
    sell(mint: string, amount: number, isEmergency?: boolean): Promise<string | null>;
    /**
     * Get entry price for position tracking
     */
    getEntryPrice(mint: string): number;
    /**
     * Register pool keys manually (e.g. from persistence)
     */
    registerPoolKeys(mint: string, keys: LiquidityPoolKeysV4): void;
    private prepareWsolAccount;
}
export {};
//# sourceMappingURL=SniperEngine.d.ts.map