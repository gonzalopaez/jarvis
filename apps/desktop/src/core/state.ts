import { EventBus } from "./event-bus";
import type { ActivityEvent, AppEvents, JarvisModel, JarvisState } from "./types";

export class JarvisStore {
  readonly bus = new EventBus<AppEvents>();
  private subscribers = new Set<(model: JarvisModel) => void>();
  private eventSequence = 0;

  private model: JarvisModel = {
    state: "idle",
    telemetry: null,
    telemetryOnline: false,
    agents: [
      { id: "codex", label: "CODEX CORE", state: "staged", detail: "MOCK ADAPTER", simulated: true },
      { id: "voice", label: "VOICE ENGINE", state: "staged", detail: "STAGED", simulated: true },
      { id: "n8n", label: "N8N", state: "staged", detail: "NOT CONNECTED", simulated: true },
      { id: "memory", label: "MEMORY", state: "staged", detail: "STAGED", simulated: true },
      { id: "monitor", label: "SYSTEM MONITOR", state: "active", detail: "INITIALIZING", simulated: false },
      { id: "proxmox", label: "PROXMOX", state: "staged", detail: "FUTURE CONNECTOR", simulated: true },
    ],
    activity: [],
    userTranscript: "Awaiting operator input.",
    jarvisTranscript: "Local system interface initialized.",
    developerControls: true,
  };

  constructor() {
    this.bus.on("telemetry.updated", (telemetry) => {
      this.model.telemetry = telemetry;
      this.model.telemetryOnline = true;
      const monitor = this.model.agents.find((agent) => agent.id === "monitor");
      if (monitor) {
        monitor.state = "active";
        monitor.detail = "LIVE DATA";
      }
      this.publish();
    });
    this.bus.on("telemetry.failed", ({ message }) => {
      this.model.telemetryOnline = false;
      const monitor = this.model.agents.find((agent) => agent.id === "monitor");
      if (monitor) {
        monitor.state = "warning";
        monitor.detail = "DATA LINK LOST";
      }
      this.addActivity("TELEMETRY", message, "error");
    });
  }

  snapshot(): JarvisModel {
    return structuredClone(this.model);
  }

  subscribe(listener: (model: JarvisModel) => void): () => void {
    this.subscribers.add(listener);
    listener(this.snapshot());
    return () => this.subscribers.delete(listener);
  }

  setState(state: JarvisState, simulated = false): void {
    if (state === this.model.state) return;
    const previous = this.model.state;
    this.model.state = state;
    this.updateAgentForState(state);
    this.bus.emit("state.changed", { state, previous });
    this.addActivity(
      state.startsWith("codex") ? "CODEX" : state.startsWith("n8n") ? "N8N" : "JARVIS",
      `STATE CHANGED: ${state.toUpperCase().replace(/-/g, " ")}`,
      state === "error" ? "error" : state.includes("warning") || state.includes("authorization") ? "warning" : "info",
      simulated,
    );
    this.publish();
  }

  setTranscript(role: "user" | "jarvis", text: string): void {
    if (role === "user") this.model.userTranscript = text;
    else this.model.jarvisTranscript = text;
    this.bus.emit("transcript.updated", { role, text });
    this.publish();
  }

  toggleDeveloperControls(): void {
    this.model.developerControls = !this.model.developerControls;
    this.publish();
  }

  addActivity(
    component: string,
    event: string,
    severity: ActivityEvent["severity"] = "info",
    simulated = false,
  ): void {
    const activity: ActivityEvent = {
      id: `${Date.now()}-${this.eventSequence++}`,
      timestamp: new Date(),
      component,
      event,
      severity,
      simulated,
    };
    this.model.activity = [activity, ...this.model.activity].slice(0, 28);
    this.bus.emit("activity.created", activity);
    this.publish();
  }

  private updateAgentForState(state: JarvisState): void {
    const codex = this.model.agents.find((agent) => agent.id === "codex");
    const n8n = this.model.agents.find((agent) => agent.id === "n8n");
    if (codex) {
      codex.state = state.startsWith("codex") ? "active" : "staged";
      codex.detail = state.startsWith("codex") ? state.split("-")[1].toUpperCase() : "MOCK ADAPTER";
    }
    if (n8n) {
      n8n.state = state === "n8n-executing" ? "active" : "staged";
      n8n.detail = state === "n8n-executing" ? "EXECUTING / MOCK" : "NOT CONNECTED";
    }
  }

  private publish(): void {
    const snapshot = this.snapshot();
    this.subscribers.forEach((listener) => listener(snapshot));
  }
}
