import { Whitelist } from "../services/Whitelist";
import { SentientBrain } from "./SentientBrain";
import { MigrationListener } from "./MigrationListener";
import { PumpPreCog } from "./PumpPreCog";
export declare class Dashboard {
    private app;
    private server;
    private port;
    private whitelist;
    private brain;
    private migrationListener;
    private pumpPreCog;
    private static logs;
    constructor(port: number, whitelist: Whitelist, brain: SentientBrain, migrationListener: MigrationListener, pumpPreCog: PumpPreCog);
    static log(msg: string): void;
    private setupRoutes;
    start(): void;
    private renderDashboard;
}
//# sourceMappingURL=Dashboard.d.ts.map