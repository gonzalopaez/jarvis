import { invoke } from "@tauri-apps/api/core";
import type { EventBus } from "../core/event-bus";
import type { AppEvents, TelemetrySnapshot } from "../core/types";

export class TelemetryClient {
  private timer: number | null = null;
  private polling = false;

  constructor(
    private readonly bus: EventBus<AppEvents>,
    private readonly intervalMs = 1200,
  ) {}

  start(): void {
    if (this.timer !== null) return;
    void this.poll();
    this.timer = window.setInterval(() => void this.poll(), this.intervalMs);
  }

  stop(): void {
    if (this.timer !== null) window.clearInterval(this.timer);
    this.timer = null;
  }

  private async poll(): Promise<void> {
    if (this.polling) return;
    this.polling = true;
    try {
      const snapshot = await invoke<TelemetrySnapshot>("get_system_telemetry");
      this.bus.emit("telemetry.updated", snapshot);
    } catch (error) {
      this.bus.emit("telemetry.failed", { message: String(error) });
    } finally {
      this.polling = false;
    }
  }
}
