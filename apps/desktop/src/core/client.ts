import type { EventBus } from "./event-bus";
import type { AppEvents, CoreConversation, CoreHealth } from "./types";
import type { JarvisRuntimeClient } from "../runtime/client";

export class CoreClient {
  private timer: number | null = null;
  private checking = false;

  constructor(
    private readonly bus: EventBus<AppEvents>,
    private readonly runtime: JarvisRuntimeClient,
    private readonly intervalMs = 15_000,
  ) {}

  start(): void {
    if (this.timer !== null) return;
    void this.checkHealth();
    this.timer = window.setInterval(() => void this.checkHealth(), this.intervalMs);
  }

  stop(): void {
    if (this.timer !== null) window.clearInterval(this.timer);
    this.timer = null;
  }

  async conversation(message: string): Promise<CoreConversation> {
    return this.runtime.conversation(message);
  }

  private async checkHealth(): Promise<void> {
    if (this.checking) return;
    this.checking = true;
    try {
      const health: CoreHealth = await this.runtime.coreHealth();
      this.bus.emit("core.health.updated", health);
    } catch (error) {
      this.bus.emit("core.health.failed", { message: String(error) });
    } finally {
      this.checking = false;
    }
  }
}
