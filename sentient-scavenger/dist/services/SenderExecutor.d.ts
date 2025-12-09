import { Connection, Keypair, Transaction, VersionedTransaction } from "@solana/web3.js";
export declare class SenderExecutor {
    private connection;
    private keypair;
    private senderUrl;
    constructor(connection: Connection, keypair: Keypair);
    executeAndConfirm(transaction: Transaction | VersionedTransaction, signTransaction?: boolean): Promise<string | null>;
}
//# sourceMappingURL=SenderExecutor.d.ts.map