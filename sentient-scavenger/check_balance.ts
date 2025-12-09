import "dotenv/config";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { getAssociatedTokenAddress } from "@solana/spl-token";
import bs58 from "bs58";
import { WSOL_MINT } from "./src/config";

async function check() {
    const privateKeyString = process.env.SOLANA_PRIVATE_KEY;
    if (!privateKeyString) throw new Error("No private key");

    let keypair;
    if (privateKeyString.startsWith("[")) {
        keypair = Keypair.fromSecretKey(new Uint8Array(JSON.parse(privateKeyString)));
    } else {
        keypair = Keypair.fromSecretKey(bs58.decode(privateKeyString));
    }

    const connection = new Connection(process.env.SOLANA_RPC_URL || "");
    const pubkey = keypair.publicKey;

    console.log("Wallet Address:", pubkey.toBase58());

    const solBalance = await connection.getBalance(pubkey);
    console.log("SOL Balance:", solBalance / 1e9);

    const wsolAta = await getAssociatedTokenAddress(new PublicKey(WSOL_MINT), pubkey);
    console.log("wSOL ATA:", wsolAta.toBase58());

    try {
        const wsolBal = await connection.getTokenAccountBalance(wsolAta);
        console.log("wSOL Balance:", wsolBal.value.uiAmount);
    } catch (e) {
        console.log("wSOL Account does not exist or has no balance.");
    }
}

check().catch(console.error);
