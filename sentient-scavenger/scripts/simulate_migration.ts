import "dotenv/config";
import {
  Connection,
  Keypair,
  PublicKey,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import bs58 from "bs58";
import { MARKET_STATE_LAYOUT_V3 } from "@raydium-io/raydium-sdk";
import { getAssociatedTokenAddressSync, TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID } from "@solana/spl-token";
import BN from "bn.js";

const PUMP_PROGRAM_ID = new PublicKey(
  "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"
);
const DEFAULT_SIMULATION_MINT =
  process.env.SIMULATION_MINT ||
  "CzLSujWBLFsSjncfkh59rUFqvafWcY5tzedWJSuypump";

// Force DRY_RUN and RELAX_FILTERS for this script
process.env.DRY_RUN = "true";
process.env.RELAX_FILTERS = "true";

// Now import modules that use config
import { MigrationListener } from "../src/core/MigrationListener";
import { SniperEngine } from "../src/core/SniperEngine";
import { SentientBrain } from "../src/core/SentientBrain";
import { JitoExecutor } from "../src/services/JitoExecutor";
import { initializeBlockhashManager } from "../src/services/BlockhashManager";
import {
  DRY_RUN,
  RELAX_FILTERS,
  PUMP_MIGRATION_AUTH,
  RAYDIUM_V4_PROGRAM,
  WSOL_MINT,
  SYSTEM_PROGRAM,
} from "../src/config";

async function runSimulation() {
    console.log("🧪 Starting Simulation...");
    console.log(`DRY_RUN: ${DRY_RUN}`);
    console.log(`RELAX_FILTERS: ${RELAX_FILTERS}`);

    if (!DRY_RUN) {
        console.error("❌ ABORT: Must be in DRY_RUN mode for simulation.");
        process.exit(1);
    }

    // Setup connection
    const rpcUrl = process.env.SOLANA_RPC_URL;
    if (!rpcUrl) {
        throw new Error("SOLANA_RPC_URL must be set for simulation.");
    }

    const connection = new Connection(rpcUrl);
    const wallet = loadKeypairFromEnv(process.env.SOLANA_PRIVATE_KEY);

    // Initialize services
    await initializeBlockhashManager(connection);
    const jitoExecutor = new JitoExecutor(connection, wallet);
    const sniperEngine = new SniperEngine(connection, wallet, jitoExecutor);
    const sentientBrain = new SentientBrain(connection, process.env.OPENAI_API_KEY || "", sniperEngine, wallet.publicKey);
    
    // Initialize Listener
    const listener = new MigrationListener(connection, sniperEngine, sentientBrain, process.env.SOLANA_RPC_URL || "");

    console.log("Creating mock transaction...");
    const simulationMint = new PublicKey(DEFAULT_SIMULATION_MINT);
    const [bondingCurve] = PublicKey.findProgramAddressSync(
        [Buffer.from("bonding-curve"), simulationMint.toBuffer()],
        PUMP_PROGRAM_ID
    );
    const bondingCurveVault = getAssociatedTokenAddressSync(
        simulationMint,
        bondingCurve,
        true,
        TOKEN_PROGRAM_ID,
        ASSOCIATED_TOKEN_PROGRAM_ID
    );

    console.log("----------------------------------------------------");
    console.log("✅ Simulation Targets");
    console.log("   Mint:          ", simulationMint.toBase58());
    console.log("   Bonding Curve: ", bondingCurve.toBase58());
    console.log("   Curve ATA:     ", bondingCurveVault.toBase58());
    console.log("----------------------------------------------------");
    
    // Generate random keys for the pool
    const marketId = Keypair.generate().publicKey;
    const baseMint = simulationMint;
    const quoteMint = new PublicKey(WSOL_MINT);
    const lpMint = Keypair.generate().publicKey;
    const poolId = Keypair.generate().publicKey;
    const authority = Keypair.generate().publicKey;
    const openOrders = Keypair.generate().publicKey;
    const baseVault = Keypair.generate().publicKey;
    const quoteVault = Keypair.generate().publicKey;
    const targetOrders = Keypair.generate().publicKey;
    const marketProgramId = Keypair.generate().publicKey;

    // Mock Market State
    const mockMarketState = Buffer.alloc(MARKET_STATE_LAYOUT_V3.span);
    MARKET_STATE_LAYOUT_V3.encode({
        blob1: Buffer.alloc(5),
        blob2: Buffer.alloc(5),
        ownAddress: marketId,
        vaultSignerNonce: new BN(0),
        baseMint: baseMint,
        quoteMint: quoteMint,
        baseVault: baseVault, // Using random keys for vaults as placeholders
        baseDepositsTotal: new BN(0),
        baseFeesAccrued: new BN(0),
        quoteVault: quoteVault,
        quoteDepositsTotal: new BN(0),
        quoteFeesAccrued: new BN(0),
        quoteDustThreshold: new BN(0),
        requestQueue: Keypair.generate().publicKey,
        eventQueue: Keypair.generate().publicKey,
        bids: Keypair.generate().publicKey,
        asks: Keypair.generate().publicKey,
        baseLotSize: new BN(1),
        quoteLotSize: new BN(1),
        feeRateBps: new BN(0),
        referrerRebatesAccrued: new BN(0),
        blob3: Buffer.alloc(7),
    } as any, mockMarketState);

    // Monkey patch getAccountInfo to return market state
    const originalGetAccountInfo = connection.getAccountInfo.bind(connection);
    connection.getAccountInfo = async (pubkey: PublicKey, commitment?: any) => {
        if (pubkey.equals(marketId)) {
            console.log("⚡ Mocking getAccountInfo for Market ID...");
            return {
                data: mockMarketState,
                executable: false,
                lamports: 0,
                owner: marketProgramId,
                rentEpoch: 0
            };
        }
        // Mock Vaults for calculateAmountOut
        if (pubkey.equals(baseVault) || pubkey.equals(quoteVault)) {
             console.log("⚡ Mocking getAccountInfo for Vault...");
             // Create a mock token account buffer
             // Layout: mint (32), owner (32), amount (8), delegate (36), state (1), isNative (12), delegatedAmount (8), closeAuthority (36)
             // We only care about amount at offset 64
             const buffer = Buffer.alloc(165);
             // Set amount to something large to avoid "insufficient liquidity"
             const amount = new BN(1000000000000); // 1000 tokens/SOL
             const amountBuffer = amount.toArrayLike(Buffer, 'le', 8);
             amountBuffer.copy(buffer, 64);
             
             return {
                data: buffer,
                executable: false,
                lamports: 0,
                owner: TOKEN_PROGRAM_ID,
                rentEpoch: 0
             };
        }

        return originalGetAccountInfo(pubkey, commitment);
    };

    const mockTx = {
        transaction: {
            message: {
                accountKeys: [
                    { pubkey: new PublicKey(PUMP_MIGRATION_AUTH), signer: true }, // 0: PUMP_AUTH
                    { pubkey: new PublicKey(RAYDIUM_V4_PROGRAM), signer: false }, // 1: Raydium Program
                    { pubkey: new PublicKey(WSOL_MINT), signer: false }, // 2: WSOL
                    { pubkey: poolId, signer: false }, // 3: Pool ID
                    { pubkey: authority, signer: false }, // 4: Authority
                    { pubkey: openOrders, signer: false }, // 5: Open Orders
                    { pubkey: lpMint, signer: false }, // 6: LP Mint
                    { pubkey: baseMint, signer: false }, // 7: Base Mint
                    { pubkey: quoteMint, signer: false }, // 8: Quote Mint
                    { pubkey: baseVault, signer: false }, // 9: Base Vault
                    { pubkey: quoteVault, signer: false }, // 10: Quote Vault
                    { pubkey: targetOrders, signer: false }, // 11: Target Orders
                    { pubkey: Keypair.generate().publicKey, signer: false }, // 12: Config
                    { pubkey: Keypair.generate().publicKey, signer: false }, // 13: Fee Dest
                    { pubkey: marketProgramId, signer: false }, // 14: Market Program
                    { pubkey: marketId, signer: false }, // 15: Market ID
                ],
                instructions: [
                    {
                        programId: new PublicKey(RAYDIUM_V4_PROGRAM),
                        accounts: [
                            new PublicKey(TOKEN_PROGRAM_ID), // 0: Token Program
                            new PublicKey(SYSTEM_PROGRAM), // 1: System Program
                            new PublicKey(SYSVAR_RENT_PUBKEY), // 2: Rent
                            poolId, // 3
                            authority, // 4
                            openOrders, // 5
                            lpMint, // 6
                            baseMint, // 7
                            quoteMint, // 8
                            baseVault, // 9
                            quoteVault, // 10
                            targetOrders, // 11
                            Keypair.generate().publicKey, // 12
                            Keypair.generate().publicKey, // 13
                            marketProgramId, // 14
                            marketId, // 15
                        ],
                        data: bs58.encode(Buffer.from([1])) // initialize2 discriminator
                    }
                ]
            }
        },
        meta: {
            logMessages: [
                `Program ${RAYDIUM_V4_PROGRAM} invoke [1]`,
                "Program log: initialize2: ...",
                `Program ${RAYDIUM_V4_PROGRAM} success`
            ],
            postTokenBalances: [
                { 
                    mint: baseMint.toBase58(), 
                    uiTokenAmount: { decimals: 6, amount: "0", uiAmount: 0, uiAmountString: "0" },
                    owner: authority.toBase58(),
                    programId: TOKEN_PROGRAM_ID.toBase58()
                },
                { 
                    mint: quoteMint.toBase58(), 
                    uiTokenAmount: { decimals: 9, amount: "0", uiAmount: 0, uiAmountString: "0" },
                    owner: authority.toBase58(),
                    programId: TOKEN_PROGRAM_ID.toBase58()
                },
                { 
                    mint: lpMint.toBase58(), 
                    uiTokenAmount: { decimals: 9, amount: "0", uiAmount: 0, uiAmountString: "0" },
                    owner: authority.toBase58(),
                    programId: TOKEN_PROGRAM_ID.toBase58()
                }
            ]
        }
    };

    // Monkey patch connection
    connection.getParsedTransaction = async () => {
        console.log("⚡ Mocking getParsedTransaction response...");
        return mockTx as any;
    };

    console.log("🚀 Triggering processMigrationLog...");
    
    try {
        await (listener as any).processMigrationLog("mock_signature", 123456);
    } catch (e) {
        console.error("Simulation error:", e);
    }

    console.log("✅ Simulation complete.");
}

function loadKeypairFromEnv(secret?: string): Keypair {
    if (!secret) {
        throw new Error("SOLANA_PRIVATE_KEY must be set for simulation.");
    }

    if (secret.trim().startsWith("[")) {
        return Keypair.fromSecretKey(new Uint8Array(JSON.parse(secret)));
    }

    return Keypair.fromSecretKey(bs58.decode(secret));
}

function createMockVaultAccountBuffer(amount: bigint): Buffer {
    const buffer = Buffer.alloc(165);
    const amountBuffer = Buffer.alloc(8);
    amountBuffer.writeBigUInt64LE(amount);
    amountBuffer.copy(buffer, 64);
    return buffer;
}

runSimulation().catch(console.error);
