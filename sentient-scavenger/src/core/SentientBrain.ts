import { Connection, PublicKey, Keypair } from "@solana/web3.js";
import { getAssociatedTokenAddress } from "@solana/spl-token";
import { OpenAI } from "openai";
import * as fs from "fs";
import * as path from "path";
import BN from "bn.js";
import { 
  PRICE_POLL_INTERVAL,
  AI_SCORE_IMMEDIATE_SELL,
  AI_SCORE_HOLD_LONG,
  TAKE_PROFIT_LOW,
  TAKE_PROFIT_HIGH,
  STOP_LOSS_LOW,
  STOP_LOSS_HIGH,
  DRY_RUN,
  DEFAULT_TOKEN_DECIMALS,
  MOONBAG_THRESHOLD_SOL,
  MOONBAG_SELL_PCT,
} from "../config";
import { getTokenPrice, getPoolPrice } from "../utils/raydium";
import { SniperEngine } from "./SniperEngine";
import { LiquidityPoolKeysV4 } from "@raydium-io/raydium-sdk";

const STATE_FILE = path.join(process.cwd(), "bot_state.json");

interface TradePosition {
  mint: string;
  entryPrice: number;
  entryTime: number;
  amount: number;
  aiScore: number;
  aiAnalysis: string;
  status: "open" | "closed";
  poolKeys?: LiquidityPoolKeysV4;
}

export class SentientBrain {
  private connection: Connection;
  private openaiClient: OpenAI;
  private sniperEngine: SniperEngine;
  private walletPublicKey: PublicKey;
  private activePositions: Map<string, TradePosition> = new Map();
  private monitoringIntervals: Map<string, NodeJS.Timeout> = new Map();

  constructor(
    connection: Connection,
    openaiApiKey: string,
    sniperEngine: SniperEngine,
    walletPublicKey: PublicKey
  ) {
    this.connection = connection;
    this.openaiClient = new OpenAI({ apiKey: openaiApiKey });
    this.sniperEngine = sniperEngine;
    this.walletPublicKey = walletPublicKey;
  }

  /**
   * Load state from disk
   */
  async loadState(): Promise<void> {
    try {
      if (!fs.existsSync(STATE_FILE)) {
        console.log("📂 No state file found, starting fresh.");
        return;
      }

      const data = fs.readFileSync(STATE_FILE, "utf-8");
      const parsed = JSON.parse(data, (key, value) => {
        if (value && value._type === "PublicKey") return new PublicKey(value.value);
        if (value && value._type === "BN") return new BN(value.value, 16);
        return value;
      });

      if (parsed.activePositions) {
        for (const [mint, pos] of Object.entries(parsed.activePositions)) {
          const position = pos as TradePosition;
          this.activePositions.set(mint, position);
          
          // Restore pool keys to SniperEngine
          if (position.poolKeys) {
            this.sniperEngine.registerPoolKeys(mint, position.poolKeys);
          }

          // Resume monitoring if open
          if (position.status === "open") {
            console.log(`🔄 Resuming monitor for ${mint}`);
            this.monitorPosition(mint);
          }
        }
      }
      console.log(`✅ State loaded: ${this.activePositions.size} positions restored.`);
    } catch (err) {
      console.error("❌ Failed to load state:", err);
    }
  }

  /**
   * Save state to disk
   */
  private saveState(): void {
    try {
      const state = {
        activePositions: Object.fromEntries(this.activePositions),
      };

      const json = JSON.stringify(state, (key, value) => {
        if (value && value.toBase58) return { _type: "PublicKey", value: value.toBase58() };
        if (value && BN.isBN(value)) return { _type: "BN", value: value.toString(16) };
        return value;
      }, 2);

      fs.writeFileSync(STATE_FILE, json);
    } catch (err) {
      console.error("❌ Failed to save state:", err);
    }
  }

  async analyzeToken(mint: string, tokenData: any): Promise<number> {
    const MAX_RETRIES = 3;
    let attempt = 0;

    while (attempt < MAX_RETRIES) {
      attempt++;
      try {
        const prompt = `Analyze this token for virality and meme potential. Be strict.
Name: ${tokenData.name}
Symbol: ${tokenData.symbol}
Description: ${tokenData.description}
Twitter: ${tokenData.twitter || "N/A"}

Rate it 1-10 for humor/virality/potential to pump. Return ONLY valid JSON: {score: <1-10>, reason: "<2 sentence reason>"}`;

        // Circuit Breaker: 15s Timeout (Increased for reliability)
        const timeoutPromise = new Promise<any>((_, reject) =>
          setTimeout(() => reject(new Error("OpenAI Timeout")), 15000)
        );

        const apiPromise = this.openaiClient.chat.completions.create({
          model: "gpt-4o-mini",
          messages: [{ role: "user", content: prompt }],
          max_tokens: 100,
        });

        const response = await Promise.race([apiPromise, timeoutPromise]);

        const text = response.choices[0].message.content || "{}";
        // Clean up markdown code blocks if present
        const cleanText = text.replace(/```json/g, "").replace(/```/g, "").trim();
        
        try {
          const parsed = JSON.parse(cleanText);
          const score = Math.max(1, Math.min(10, parsed.score));
          console.log(
            `🧠 AI Analysis for ${tokenData.name}: Score ${score}/10`
          );
          console.log(`   └─ ${parsed.reason}`);
          return score;
        } catch (parseError) {
          console.warn(`⚠️ Failed to parse AI response (Attempt ${attempt}):`, parseError);
          throw new Error("JSON Parse Error");
        }
      } catch (err: any) {
        console.warn(`⚠️ AI Analysis attempt ${attempt}/${MAX_RETRIES} failed: ${err.message}`);
        
        if (attempt < MAX_RETRIES) {
          // Exponential backoff: 1s, 2s, 4s
          const delay = 1000 * Math.pow(2, attempt - 1);
          console.log(`   ⏳ Retrying in ${delay}ms...`);
          await new Promise(resolve => setTimeout(resolve, delay));
        } else {
          console.error("❌ All AI attempts failed. Defaulting to score 5.");
          return 5; // Safe default after all retries fail
        }
      }
    }
    return 5;
  }

  async recordPosition(
    mint: string,
    aiScore: number,
    analysis: string,
    poolKeys?: LiquidityPoolKeysV4
  ): Promise<void> {
    let finalEntryPrice = 1.0;
    let finalAmount = 0;

    // 1. Fetch Real Balance (wait for bundle fill)
    try {
      const { amountRaw } = await this.fetchTokenBalanceWithRetry(
        mint,
        30,
        500,
        true
      );
      if (amountRaw > BigInt(0)) {
        finalAmount = this.bigIntToSafeNumber(amountRaw);
        console.log(`   └─ Confirmed on-chain balance: ${finalAmount}`);
      } else {
        console.warn(
          `   └─ No tokens detected for ${mint} after waiting. Skipping position.`
        );
        return;
      }
    } catch (e) {
      console.warn(`   └─ Failed to fetch balance for ${mint}: ${e}`);
      return;
    }
    
    // 2. Fetch Real Price
    if (poolKeys) {
      try {
        const price = await getPoolPrice(this.connection, poolKeys);
        if (price > 0) {
          finalEntryPrice = price;
          console.log(`   └─ Fetched real entry price: ${finalEntryPrice}`);
        }
      } catch (e) {
        console.warn(`   └─ Failed to fetch initial price, using default: ${e}`);
      }
    }

    const position: TradePosition = {
      mint,
      entryPrice: finalEntryPrice,
      entryTime: Date.now(),
      amount: finalAmount,
      aiScore,
      aiAnalysis: analysis,
      status: "open",
      poolKeys
    };

    this.activePositions.set(mint, position);
    this.saveState();
    console.log(
      `📊 Position recorded: ${mint} | Score: ${aiScore}/10 | Entry: $${finalEntryPrice.toFixed(6)}`
    );

    // Determine exit strategy based on score
    if (aiScore < AI_SCORE_IMMEDIATE_SELL) {
      console.log(`   └─ Low score (${aiScore}), selling IMMEDIATELY`);
      await this.sell(mint, "Low AI score", true);
    } else {
      // Start monitoring this position
      this.monitorPosition(mint);
    }
  }

  private monitorPosition(mint: string): void {
    const position = this.activePositions.get(mint);
    if (!position) return;

    console.log(`   └─ Starting price monitor for ${mint}...`);

    const interval = setInterval(async () => {
      if (position.status === "closed") {
        clearInterval(interval);
        this.monitoringIntervals.delete(mint);
        return;
      }

      const currentPrice = await this.getPriceFromRpc(mint);
      
      // Skip if price fetch failed
      if (currentPrice === null || currentPrice === 0) return;

      const priceChange = (currentPrice - position.entryPrice) / position.entryPrice;
      const percentChange = priceChange * 100;

      // Determine thresholds based on AI score
      const takeProfit =
        position.aiScore > AI_SCORE_HOLD_LONG
          ? TAKE_PROFIT_HIGH
          : TAKE_PROFIT_LOW;
      const stopLoss =
        position.aiScore > AI_SCORE_HOLD_LONG ? STOP_LOSS_HIGH : STOP_LOSS_LOW;

      // Short log every 10 seconds
      if (Math.random() < 0.1) {
        console.log(
          `📈 ${mint}: ${percentChange.toFixed(2)}% | TP: ${(takeProfit * 100).toFixed(0)}% | SL: ${(stopLoss * 100).toFixed(0)}%`
        );
      }

      // Check if TP or SL hit
      if (priceChange >= takeProfit) {
        console.log(
          `🚀 TAKE PROFIT HIT: ${mint} | Gain: +${percentChange.toFixed(2)}%`
        );
        // Determine if we want to moonbag
        let sellAmount = position.amount;
        const estValueSol =
          (position.amount / Math.pow(10, DEFAULT_TOKEN_DECIMALS)) *
          currentPrice;
        if (estValueSol >= MOONBAG_THRESHOLD_SOL) {
          sellAmount = Math.floor(position.amount * MOONBAG_SELL_PCT);
          console.log(
            `   └─ Moonbag mode: selling ${MOONBAG_SELL_PCT * 100}% (~${sellAmount} units)`
          );
        }

        position.status = "closed";
        clearInterval(interval);
        this.monitoringIntervals.delete(mint);
        await this.sell(mint, "Take profit", false, sellAmount);
      } else if (priceChange <= stopLoss) {
        console.log(
          `🛑 STOP LOSS HIT: ${mint} | Loss: ${percentChange.toFixed(2)}%`
        );
        position.status = "closed";
        clearInterval(interval);
        this.monitoringIntervals.delete(mint);
        await this.sell(mint, "Stop loss");
      }
    }, PRICE_POLL_INTERVAL);

    this.monitoringIntervals.set(mint, interval);
  }

  private async getPriceFromRpc(mint: string): Promise<number | null> {
    try {
      const position = this.activePositions.get(mint);
      
      // Use local Raydium calculation (FAST & REAL)
      if (position?.poolKeys) {
        return await getPoolPrice(this.connection, position.poolKeys);
      }

      // Fallback for DRY_RUN or missing keys
      if (DRY_RUN) {
        const randomWalk = (Math.random() - 0.49) * 0.02; 
        const basePrice = position?.entryPrice || 1.0;
        return basePrice * (1 + randomWalk);
      }

      console.warn(`⚠️ No pool keys for ${mint}, cannot fetch real price.`);
      return null;
    } catch (err) {
      console.error(`getPriceFromRpc failed for ${mint}:`, err);
      return null;
    }
  }

  async sell(
    mint: string,
    reason: string = "Manual",
    isEmergency: boolean = false,
    overrideAmount?: number
  ): Promise<boolean> {
    try {
      const position = this.activePositions.get(mint);
      if (!position) {
        console.warn(`Position not found for ${mint}`);
        return false;
      }

      if (position.status === "closed") {
        console.log(`Position already closed: ${mint}`);
        return true;
      }

      console.log(`💰 SELLING ${mint} | Reason: ${reason} ${isEmergency ? "(PANIC)" : ""}`);

      if (DRY_RUN) {
        console.log(`   └─ (DRY RUN) Would execute market sell`);
        position.status = "closed";
        this.activePositions.delete(mint);
        this.saveState();
        return true;
      }

      let amountRaw: bigint = BigInt(overrideAmount ?? position.amount);
      try {
        const balance = await this.fetchTokenBalanceWithRetry(
          mint,
          isEmergency ? 1 : 5
        );
        amountRaw = balance.amountRaw;
      } catch (err) {
        console.warn(`   └─ Could not refresh ${mint} balance before sell:`, err);
      }

      if (amountRaw <= BigInt(0)) {
        console.warn(`   └─ No tokens left in ATA for ${mint}. Marking closed.`);
        position.status = "closed";
        this.activePositions.delete(mint);
        this.saveState();
        return true;
      }

      const amountNumber = this.bigIntToSafeNumber(amountRaw);

      // Execute real sell transaction via SniperEngine
      console.log(`   └─ Executing sell via SniperEngine...`);
      const bundleId = await this.sniperEngine.sell(
        mint,
        amountNumber,
        isEmergency
      );

      if (bundleId) {
        console.log(`✅ Sell confirmed: ${bundleId}`);
        position.status = "closed";
        this.activePositions.delete(mint);
        this.saveState();
        return true;
      } else {
        console.error("❌ Sell failed to execute");
        return false;
      }
    } catch (err) {
      console.error("Sell failed:", err);
      return false;
    }
  }

  /**
   * Get all active positions
   */
  getActivePositions(): TradePosition[] {
    return Array.from(this.activePositions.values()).filter(
      (p) => p.status === "open"
    );
  }

  /**
   * Close all positions (emergency exit)
   */
  async closeAll(): Promise<void> {
    console.log("🛑 Emergency: Closing all positions...");
    for (const [mint, position] of this.activePositions) {
      if (position.status === "open") {
        await this.sell(mint, "Emergency close");
      }
    }
  }

  private async fetchTokenBalanceWithRetry(
    mint: string,
    retries = 5,
    delayMs = 500,
    requirePositive = false
  ): Promise<{ amountRaw: bigint; decimals: number }> {
    const ata = await getAssociatedTokenAddress(
      new PublicKey(mint),
      this.walletPublicKey
    );

    for (let i = 0; i < retries; i++) {
      try {
        const bal = await this.connection.getTokenAccountBalance(ata);
        const raw = BigInt(bal.value.amount || "0");
        const decimals = bal.value.decimals ?? 0;
        if (!requirePositive || raw > BigInt(0) || i === retries - 1) {
          return { amountRaw: raw, decimals };
        }
      } catch (err) {
        if (i === retries - 1) throw err;
      }
      await new Promise((resolve) => setTimeout(resolve, delayMs));
    }

    return { amountRaw: BigInt(0), decimals: 0 };
  }

  private bigIntToSafeNumber(value: bigint): number {
    const max = BigInt(Number.MAX_SAFE_INTEGER);
    if (value > max) {
      console.warn(
        `Value ${value.toString()} exceeds MAX_SAFE_INTEGER. Truncating.`
      );
      return Number(max);
    }
    return Number(value);
  }
}
