import { PublicKey } from "@solana/web3.js";
import { RAYDIUM_V4_PROGRAM, TOKEN_PROGRAM } from "./src/config";

console.log("Testing PublicKey creation...");

import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
console.log("SPL Token Program ID:", TOKEN_PROGRAM_ID.toBase58());

const bs58 = require('bs58');
const decoded = bs58.decode("TokenkegQfeZyiNwAJsyFbPVwwQQfk5P5pQ69YQ92");
console.log("TokenProgram decoded length:", decoded.length);


const raydium = bs58.decode("675kPPazMwLrhu35sVdGq71g3nF8Fa4b4vJ9D5L9x");
console.log("Raydium decoded length:", raydium.length);



try {
    const tokenProgram = new PublicKey("TokenkegQfeZyiNwAJsyFbPVwwQQfk5P5pQ69YQ92");
    console.log("Success hardcoded TokenProgram:", tokenProgram.toBase58());
} catch (e) {
    console.error("Failed hardcoded TokenProgram:", e);
}

try {
    const tokenProgramConst = new PublicKey(TOKEN_PROGRAM);
    console.log("Success constant TokenProgram:", tokenProgramConst.toBase58());
} catch (e) {
    console.error("Failed constant TokenProgram:", e);
}

try {
    const raydium = new PublicKey(RAYDIUM_V4_PROGRAM);
    console.log("Success Raydium:", raydium.toBase58());
} catch (e) {
    console.error("Failed Raydium:", e);
}

// import bs58 from 'bs58';
// console.log("bs58 test:", bs58.encode(Buffer.from("test")));
