"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.initializeBlockhashManager = initializeBlockhashManager;
exports.getCachedBlockhash = getCachedBlockhash;
const config_1 = require("../config");
let latestBlockHash = null;
async function initializeBlockhashManager(connection) {
    // Initial fetch
    await updateBlockhash(connection);
    // Start polling loop with backoff
    pollBlockhash(connection);
}
async function pollBlockhash(connection) {
    try {
        await updateBlockhash(connection);
        // Success: wait standard interval
        setTimeout(() => pollBlockhash(connection), config_1.BLOCKHASH_POLL_INTERVAL);
    }
    catch (err) {
        // Error: wait longer (backoff)
        // console.error("Blockhash poll failed, backing off...");
        setTimeout(() => pollBlockhash(connection), config_1.BLOCKHASH_POLL_INTERVAL * 5);
    }
}
async function updateBlockhash(connection) {
    try {
        const bh = await connection.getLatestBlockhash("processed");
        latestBlockHash = {
            blockhash: bh.blockhash,
            lastValidBlockHeight: bh.lastValidBlockHeight,
            timestamp: Date.now(),
        };
    }
    catch (err) {
        // console.error("Blockhash update failed:", err);
    }
}
function getCachedBlockhash() {
    if (!latestBlockHash)
        return null;
    // Return cached if less than 2 seconds old
    if (Date.now() - latestBlockHash.timestamp < 2000) {
        return {
            blockhash: latestBlockHash.blockhash,
            lastValidBlockHeight: latestBlockHash.lastValidBlockHeight,
        };
    }
    // Stale, but return anyway (it will be refreshed on next poll)
    return {
        blockhash: latestBlockHash.blockhash,
        lastValidBlockHeight: latestBlockHash.lastValidBlockHeight,
    };
}
//# sourceMappingURL=BlockhashManager.js.map