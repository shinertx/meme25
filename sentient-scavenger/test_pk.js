const { PublicKey } = require("@solana/web3.js");
const bs58 = require('bs58');

console.log("Testing bs58 v6...");
try {
  const decoded = bs58.decode("675kPPazMwLrhu35sVdGq71g3nF8Fa4b4vJ9D5L9x");
  console.log("bs58 v6 decode success, length:", decoded.length);
} catch (e) {
  console.log("bs58 v6 decode fail", e);
}

console.log("Testing PublicKey...");
try {
  const pk = new PublicKey("675kPPazMwLrhu35sVdGq71g3nF8Fa4b4vJ9D5L9x");
  console.log("Success:", pk.toBase58());
} catch (err) {
  console.error("Failed:", err);
}
