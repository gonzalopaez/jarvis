import { EventBus } from "./event-bus";
import type { ActivityEvent, AppEvents, JarvisModel, JarvisState } from "./types";
import { normalizeAudioLevel } from "../audio/source";

export class JarvisStore {
  readonly bus = new EventBus<AppEvents>();
  private subscribers = new Set<(model: JarvisModel) => void>();
  private eventSequence = 0;

  private model: JarvisModel = {
    state: "idle",
    telemetry: null,
    telemetryOnline: false,
    agents: [
      { id: "core", label: "JARVIS CORE", state: "offline", detail: "CONNECTING", simulated: false },
      { id: "codex", label: "CODEX CORE", state: "offline", detail: "NOT CONNECTED", simulated: false },
      { id: "voice", label: "VOICE ENGINE", state: "offline", detail: "NOT CONNECTED", simulated: false },
      { id: "memory", label: "MEMORY", state: "offline", detail: "NOT CONNECTED", simulated: false },
      { id: "n8n", label: "N8N", state: "offline", detail: "NOT CONNECTED", simulated: false },
      { id: "monitor", label: "SYSTEM MONITOR", state: "offline", detail: "INITIALIZING", simulated: false },
      { id: "security", label: "SECURITY AGENT", state: "offline", detail: "NOT CONNECTED", simulated: false },
      { id: "mcp", label: "MCP GATEWAY", state: "offline", detail: "NOT CONNECTED", simulated: false },
    ],
    activity: [],
    userTranscript: "Awaiting operator input.",
    jarvisTranscript: "Local system interface initialized.",
    developerControls: true,
    audioVisualization: { source: "none", level: 0 },
    securityTelemetry: null,
    securityAlerts: [],
  };

  constructor() {
    this.bus.on("telemetry.updated", (telemetry) => {
      this.model.telemetry = telemetry;
      this.model.telemetryOnline = true;
      const monitor = this.model.agents.find((agent) => agent.id === "monitor");
      if (monitor) {
        monitor.state = "realtime";
        monitor.detail = "LIVE DATA";
      }
      this.publish();
    });
    this.bus.on("telemetry.failed", ({ message }) => {
      this.model.telemetryOnline = false;
      const monitor = this.model.agents.find((agent) => agent.id === "monitor");
      if (monitor) {
        monitor.state = "degraded";
        monitor.detail = "DATA LINK LOST";
      }
      this.addActivity("TELEMETRY", message, "error");
    });
    this.bus.on("telemetry.unavailable", ({ message }) => {
      this.model.telemetryOnline = false;
      const monitor = this.model.agents.find((agent) => agent.id === "monitor");
      if (monitor) {
        monitor.state = "offline";
        monitor.detail = "REALTIME GATEWAY PENDING";
        monitor.simulated = false;
      }
      this.addActivity("TELEMETRY", message, "info");
    });
    this.bus.on("core.health.updated", (health) => {
      if (health.components) {
        for (const component of health.components) {
          const agent = this.model.agents.find((candidate) => candidate.id === component.id);
          if (!agent) continue;
          agent.state = component.agentStatus;
          agent.detail = component.status === "healthy"
            ? `${component.version.toUpperCase()} / ${component.latencyMs ?? health.latencyMs}MS`
            : component.error?.toUpperCase() ?? component.status.toUpperCase();
          agent.simulated = false;
          if (component.latencyMs !== undefined) agent.latencyMs = component.latencyMs;
          else delete agent.latencyMs;
        }
      } else {
        const core = this.model.agents.find((agent) => agent.id === "core");
        if (core) {
          core.state = "ready";
          core.detail = `TLS / ${health.apiVersion.toUpperCase()} / ${health.latencyMs}MS`;
          core.latencyMs = health.latencyMs;
        }
      }
      if (health.state && !["microphone", "tts"].includes(this.model.audioVisualization.source)) {
        this.model.state = health.state;
      }
      this.publish();
    });
    this.bus.on("core.health.failed", ({ message }) => {
      const core = this.model.agents.find((agent) => agent.id === "core");
      if (core) {
        core.state = "offline";
        core.detail = "LINK UNAVAILABLE";
        delete core.latencyMs;
      }
      this.addActivity("CORE", message, "error");
    });
    this.bus.on("realtime.connected", () => {
      this.addActivity("REALTIME", "AUTHENTICATED WEBSOCKET CONNECTED", "success");
    });
    this.bus.on("realtime.unavailable", ({ reason }) => {
      this.addActivity("REALTIME", reason, "warning");
    });
    this.bus.on("realtime.disconnected", ({ reason }) => {
      this.addActivity("REALTIME", reason, "warning");
    });
    this.bus.on("realtime.resync.required", () => {
      this.addActivity("REALTIME", "SNAPSHOT RESYNCHRONIZATION REQUIRED", "warning");
    });
    this.bus.on("realtime.state.changed", ({ state }) => this.setState(state));
    this.bus.on("realtime.agent.changed", (component) => {
      const agent = this.model.agents.find((candidate) => candidate.id === component.id);
      if (!agent) return;
      agent.state = component.agentStatus;
      agent.detail = component.error?.toUpperCase() ?? component.status.toUpperCase();
      agent.simulated = false;
      this.publish();
    });
    this.bus.on("realtime.activity", ({ component, message, severity }) => {
      this.addActivity(component, message, severity);
    });
    this.bus.on("voice.input.level", ({ level }) => this.setAudioLevel("microphone", level));
    this.bus.on("voice.output.level", ({ level }) => this.setAudioLevel("tts", level));
    this.bus.on("security.telemetry.updated", (snapshot) => {
      this.model.securityTelemetry = snapshot;
      this.publish();
    });
    this.bus.on("security.alert", (alert) => {
      this.model.securityAlerts = [alert, ...this.model.securityAlerts.filter((item) => item.id !== alert.id)].slice(0, 50);
      this.addActivity("SECURITY", alert.title, alert.severity === "critical" || alert.severity === "high" ? "error" : "warning");
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

  setState(state: JarvisState, simulated = false, operationContext?: string): void {
    if (state === this.model.state && operationContext === this.model.operationContext) return;
    const previous = this.model.state;
    this.model.state = state;
    if (operationContext) this.model.operationContext = operationContext;
    else delete this.model.operationContext;
    this.bus.emit("state.changed", { state, previous });
    this.addActivity(
      "JARVIS",
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

  setDevAudioLevel(level: number): void {
    this.setAudioLevel("dev", level);
  }

  clearAudioLevel(): void {
    this.model.audioVisualization = { source: "none", level: 0 };
    this.publish();
  }

  private setAudioLevel(source: "microphone" | "tts" | "dev", level: number): void {
    this.model.audioVisualization = { source, level: normalizeAudioLevel(level) };
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

  private publish(): void {
    const snapshot = this.snapshot();
    this.subscribers.forEach((listener) => listener(snapshot));
  }
}
