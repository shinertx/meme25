import express from "express";
import { Server } from "http";
import { Whitelist } from "../services/Whitelist";
import { SentientBrain } from "./SentientBrain";
import { MigrationListener } from "./MigrationListener";
import { PumpPreCog } from "./PumpPreCog";

export class Dashboard {
  private app: express.Application;
  private server: Server | null = null;
  private port: number;
  
  // References to components for data fetching
  private whitelist: Whitelist;
  private brain: SentientBrain;
  private migrationListener: MigrationListener;
  private pumpPreCog: PumpPreCog;

  // Simple in-memory log buffer for the UI
  private static logs: string[] = [];

  constructor(
    port: number,
    whitelist: Whitelist,
    brain: SentientBrain,
    migrationListener: MigrationListener,
    pumpPreCog: PumpPreCog
  ) {
    this.port = port;
    this.whitelist = whitelist;
    this.brain = brain;
    this.migrationListener = migrationListener;
    this.pumpPreCog = pumpPreCog;
    this.app = express();
    this.setupRoutes();
  }

  public static log(msg: string) {
    const timestamp = new Date().toISOString().split('T')[1].split('.')[0];
    this.logs.unshift(`[${timestamp}] ${msg}`);
    if (this.logs.length > 50) this.logs.pop();
  }

  private setupRoutes() {
    this.app.get("/", (req, res) => {
      res.send(this.renderDashboard());
    });

    this.app.get("/api/status", (req, res) => {
      res.json({
        whitelistSize: this.whitelist.size(),
        activePositions: this.brain.getActivePositions().length,
        raydiumVelocity: this.migrationListener.getVelocity(), // We need to implement this
        pumpVelocity: this.pumpPreCog.getVelocity(), // We need to implement this
        logs: Dashboard.logs
      });
    });
  }

  public start() {
    this.server = this.app.listen(this.port, () => {
      console.log(`📊 Dashboard running at http://localhost:${this.port}`);
    });
  }

  private renderDashboard(): string {
    return `
      <!DOCTYPE html>
      <html>
      <head>
        <title>Sentient Scavenger Dashboard</title>
        <style>
          body { font-family: 'Courier New', monospace; background: #0d1117; color: #c9d1d9; padding: 20px; }
          .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 20px; margin-bottom: 20px; }
          .card { background: #161b22; padding: 15px; border: 1px solid #30363d; border-radius: 6px; }
          .card h3 { margin: 0 0 10px 0; color: #8b949e; font-size: 14px; }
          .card .value { font-size: 24px; font-weight: bold; color: #58a6ff; }
          .logs { background: #161b22; padding: 15px; border: 1px solid #30363d; border-radius: 6px; height: 400px; overflow-y: auto; }
          .log-entry { border-bottom: 1px solid #21262d; padding: 5px 0; font-size: 12px; }
          .success { color: #2ea043; }
          .warn { color: #d29922; }
          .error { color: #f85149; }
        </style>
        <script>
          function update() {
            fetch('/api/status')
              .then(res => res.json())
              .then(data => {
                document.getElementById('wl-size').innerText = data.whitelistSize;
                document.getElementById('active-pos').innerText = data.activePositions;
                document.getElementById('ray-vel').innerText = data.raydiumVelocity;
                document.getElementById('pump-vel').innerText = data.pumpVelocity;
                
                const logsDiv = document.getElementById('logs-container');
                logsDiv.innerHTML = data.logs.map(l => '<div class="log-entry">' + l + '</div>').join('');
              });
          }
          setInterval(update, 1000);
          window.onload = update;
        </script>
      </head>
      <body>
        <h1>🤖 Sentient Scavenger Dashboard</h1>
        
        <div class="grid">
          <div class="card">
            <h3>Whitelist Size</h3>
            <div class="value" id="wl-size">--</div>
          </div>
          <div class="card">
            <h3>Active Positions</h3>
            <div class="value" id="active-pos">--</div>
          </div>
          <div class="card">
            <h3>Raydium Velocity (Events)</h3>
            <div class="value" id="ray-vel">--</div>
          </div>
           <div class="card">
            <h3>Pump Velocity (Logs)</h3>
            <div class="value" id="pump-vel">--</div>
          </div>
        </div>

        <div class="logs">
          <h3>Recent Activity</h3>
          <div id="logs-container"></div>
        </div>
      </body>
      </html>
    `;
  }
}
