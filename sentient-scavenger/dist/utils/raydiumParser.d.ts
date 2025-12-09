import { ParsedTransactionWithMeta, Connection } from "@solana/web3.js";
import { LiquidityPoolKeysV4 } from "@raydium-io/raydium-sdk";
export declare function parseRaydiumMigration(connection: Connection, tx: ParsedTransactionWithMeta): Promise<LiquidityPoolKeysV4 | null>;
//# sourceMappingURL=raydiumParser.d.ts.map