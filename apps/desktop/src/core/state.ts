import { EventBus } from "./event-bus";
import type { ActivityEvent, AppEvents, JarvisModel, JarvisState, SecurityAlert } from "./types";
import { normalizeAudioLevel } from "../audio/source";

let alertAudioContext: AudioContext | null = null;
let pendingCriticalAlert: SecurityAlert | null = null;
let alertUnlockInstalled = false;
let pendingServerAudio: HTMLAudioElement | null = null;
let pendingServerAudioLoad: Promise<void> | null = null;

export class JarvisStore {
  readonly bus = new EventBus<AppEvents>();
  private subscribers = new Set<(model: JarvisModel) => void>();
  private eventSequence = 0;
  private announcedCriticalAlerts = new Set<string>();

  private model: JarvisModel = {
    state: "idle",
    telemetry: null,
    telemetryOnline: false,
    agents: [
      { id: "core", label: "JARVIS CORE", state: "offline", detail: "CONNECTING", simulated: false },
      { id: "codex", label: "CODEX CORE", state: "offline", detail: "NOT CONNECTED", simulated: false },
      { id: "voice", label: "VOICE SERVICE", state: "offline", detail: "NOT CONNECTED", simulated: false },
      { id: "mcp", label: "MCP GATEWAY", state: "offline", detail: "NOT CONNECTED", simulated: false },
      { id: "n8n", label: "N8N", state: "offline", detail: "NOT CONNECTED", simulated: false },
      { id: "wazuh", label: "WAZUH AGENT", state: "offline", detail: "NOT CONNECTED", simulated: false },
      { id: "proxmox", label: "PROXMOX AGENT", state: "offline", detail: "NOT INSTRUMENTED", simulated: false },
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
      this.publish();
    });
    this.bus.on("telemetry.failed", ({ message }) => {
      this.model.telemetryOnline = false;
      this.addActivity("TELEMETRY", message, "error");
    });
    this.bus.on("telemetry.unavailable", ({ message }) => {
      this.model.telemetryOnline = false;
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
      if (alert.severity === "critical" && !this.announcedCriticalAlerts.has(alert.id)) {
        this.announcedCriticalAlerts.add(alert.id);
        if (this.announcedCriticalAlerts.size > 100) {
          const first = this.announcedCriticalAlerts.values().next().value;
          if (first) this.announcedCriticalAlerts.delete(first);
        }
        announceCriticalAlert(alert);
      }
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

function announceCriticalAlert(alert: SecurityAlert): void {
  if (typeof window === "undefined") return;
  const host = alert.host || "host desconocido";
  const announcement = `Detectamos una alerta crítica en este equipo: ${host}. Tipo de alerta: ${alert.title}.`;
  void preloadServerAlertVoice(announcement);
  pendingCriticalAlert = alert;
  if (!alertUnlockInstalled) {
    alertUnlockInstalled = true;
    window.addEventListener("pointerdown", () => {
      if (pendingCriticalAlert) {
        const queued = pendingCriticalAlert;
        pendingCriticalAlert = null;
        playCriticalAlert(queued);
      }
    });
  }
  playCriticalAlert(alert);
}

function playCriticalAlert(alert: SecurityAlert): void {
  if (typeof window === "undefined") return;
  const AudioContextCtor = window.AudioContext
    ?? (window as Window & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (AudioContextCtor) {
    try {
      alertAudioContext ??= new AudioContextCtor();
      const context = alertAudioContext;
      void context.resume();
      const oscillator = context.createOscillator();
      const gain = context.createGain();
      oscillator.type = "sine";
      oscillator.frequency.value = 880;
      gain.gain.setValueAtTime(0.0001, context.currentTime);
      gain.gain.exponentialRampToValueAtTime(0.18, context.currentTime + 0.02);
      gain.gain.exponentialRampToValueAtTime(0.0001, context.currentTime + 0.28);
      oscillator.connect(gain).connect(context.destination);
      oscillator.start();
      oscillator.stop(context.currentTime + 0.3);
      oscillator.addEventListener("ended", () => undefined, { once: true });
    } catch { /* Browser autoplay policy may block the alert tone. */ }
  }
  const host = alert.host || "host desconocido";
  void playServerAlertVoice(alert, `${host}`);
}

async function playServerAlertVoice(alert: SecurityAlert, host: string): Promise<void> {
  const message = `Detectamos una alerta crítica en este equipo: ${host}. Tipo de alerta: ${alert.title}.`;
  try {
    await preloadServerAlertVoice(message);
    const audio = pendingServerAudio;
    if (audio) {
      await audio.play();
      return;
    }
    // No server audio available (TTS unavailable): fall back to a visible notice.
    window.alert(message);
  } catch {
    // Keep a visible notification only when playback fails or autoplay is blocked.
    window.alert(message);
  }
}

function preloadServerAlertVoice(text: string): Promise<void> {
  if (pendingServerAudio) return Promise.resolve();
  // Coalesce concurrent callers onto a single in-flight synthesis request so a
  // critical alert never triggers two parallel POSTs to /api/v1/voice/alert.
  if (pendingServerAudioLoad) return pendingServerAudioLoad;
  pendingServerAudioLoad = (async () => {
    try {
      const session = await fetch("/api/v1/session", {
        method: "GET",
        credentials: "same-origin",
        headers: { Accept: "application/json" },
        redirect: "error",
        cache: "no-store",
      });
      if (!session.ok) return;
      const status: unknown = await session.json();
      const csrfToken = status && typeof status === "object" && !Array.isArray(status)
        ? (status as { csrf_token?: unknown }).csrf_token
        : null;
      if (typeof csrfToken !== "string" || !/^[0-9a-f]{64}$/.test(csrfToken)) return;
      const response = await fetch("/api/v1/voice/alert", {
        method: "POST",
        credentials: "same-origin",
        headers: {
          Accept: "audio/wav",
          "content-type": "application/json",
          "x-jarvis-csrf": csrfToken,
        },
        redirect: "error",
        cache: "no-store",
        body: JSON.stringify({ text }),
      });
      if (!response.ok) return;
      pendingServerAudio = new Audio(URL.createObjectURL(await response.blob()));
      pendingServerAudio.volume = 1;
      pendingServerAudio.addEventListener("ended", () => {
        if (pendingServerAudio) URL.revokeObjectURL(pendingServerAudio.src);
        pendingServerAudio = null;
      }, { once: true });
    } catch { /* UI keeps the alert visible if TTS is unavailable. */ }
  })();
  try {
    return pendingServerAudioLoad;
  } finally {
    // Allow a fresh attempt on the next alert once this one settles (a failed
    // synthesis leaves pendingServerAudio null, so callers may retry).
    void pendingServerAudioLoad.finally(() => { pendingServerAudioLoad = null; });
  }
}
