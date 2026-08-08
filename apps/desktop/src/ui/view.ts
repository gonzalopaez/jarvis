import type { JarvisModel, JarvisState, TelemetrySnapshot } from "../core/types";
import { STATE_LABELS } from "../core/types";

const stateModes: Record<JarvisState, string> = {
  idle: "SIGNAL // AMBIENT",
  listening: "VOICE // INPUT",
  thinking: "NEURAL // PROCESSING",
  routing: "INTENT // CLASSIFYING",
  "codex-analyzing": "CODEX // ANALYSIS",
  "codex-executing": "CODEX // TASK ACTIVITY",
  "n8n-executing": "N8N // WORKFLOW ACTIVITY",
  speaking: "JARVIS // OUTPUT",
  warning: "SYSTEM // ANOMALY",
  "authorization-required": "SECURITY // HOLD",
  error: "SYSTEM // FAULT",
  offline: "SIGNAL // LOST",
};

export class JarvisView {
  private readonly networkHistory: Array<[number, number]> = [];
  private wavePhase = 0;
  private animationFrame = 0;
  private currentState: JarvisState = "idle";

  constructor(private readonly root: HTMLElement) {
    this.animateWaveform();
  }

  render(model: JarvisModel): void {
    this.currentState = model.state;
    this.root.dataset.state = model.state;
    this.text("state-label", STATE_LABELS[model.state]);
    this.text("wave-mode", stateModes[model.state]);
    this.text("user-transcript", model.userTranscript);
    this.text("jarvis-transcript", model.jarvisTranscript);
    this.renderAgents(model);
    this.renderActivity(model);
    this.renderAuthorization(model.state === "authorization-required");
    document.querySelector("#dev-controls")?.classList.toggle("is-hidden", !model.developerControls);
    if (model.telemetry) this.renderTelemetry(model.telemetry);
  }

  destroy(): void {
    cancelAnimationFrame(this.animationFrame);
  }

  private renderTelemetry(data: TelemetrySnapshot): void {
    this.text("host-name", data.hostname.toUpperCase());
    this.text("uptime", formatDuration(data.uptimeSeconds));
    this.text("cpu-value", data.cpuUsage.toFixed(0));
    this.text("memory-value", data.memoryUsage.toFixed(0));
    this.text("disk-value", data.diskUsage.toFixed(0));
    this.text("memory-detail", `${formatBytes(data.memoryUsed)} / ${formatBytes(data.memoryTotal)}`);
    this.text("disk-detail", `${formatBytes(data.diskUsed)} / ${formatBytes(data.diskTotal)}`);
    this.text("network-rx", `${formatRate(data.networkRxPerSec)} ↓`);
    this.text("network-tx", `${formatRate(data.networkTxPerSec)} ↑`);
    this.text("load-vector", data.loadAverage.map((value) => value.toFixed(2)).join(" / "));
    this.text("kernel", data.kernel);
    this.setMetric("cpu", data.cpuUsage);
    this.setMetric("memory", data.memoryUsage);
    this.setMetric("disk", data.diskUsage);
    this.networkHistory.push([data.networkRxPerSec, data.networkTxPerSec]);
    if (this.networkHistory.length > 64) this.networkHistory.shift();
    this.drawNetwork();
  }

  private renderAgents(model: JarvisModel): void {
    const list = this.root.querySelector<HTMLElement>("#agent-list");
    if (!list) return;
    list.replaceChildren(
      ...model.agents.map((agent) => {
        const row = document.createElement("article");
        row.className = `agent agent-${agent.state}`;
        const signal = document.createElement("i");
        const copy = document.createElement("div");
        const label = document.createElement("strong");
        const detail = document.createElement("span");
        const status = document.createElement("b");
        label.textContent = agent.label;
        detail.textContent = `${agent.detail}${agent.simulated ? " // SIM" : ""}`;
        status.textContent = agent.state.toUpperCase();
        copy.append(label, detail);
        row.append(signal, copy, status);
        return row;
      }),
    );
  }

  private renderActivity(model: JarvisModel): void {
    const stream = this.root.querySelector<HTMLElement>("#activity-stream");
    if (!stream) return;
    stream.replaceChildren(
      ...model.activity.slice(0, 12).map((event) => {
        const row = document.createElement("div");
        row.className = `activity activity-${event.severity}`;
        const time = document.createElement("time");
        const component = document.createElement("span");
        const message = document.createElement("p");
        time.textContent = event.timestamp.toLocaleTimeString("es-AR", { hour12: false });
        component.textContent = event.component;
        message.textContent = `${event.event}${event.simulated ? " [SIM]" : ""}`;
        row.append(time, component, message);
        return row;
      }),
    );
  }

  private renderAuthorization(visible: boolean): void {
    const modal = this.root.querySelector<HTMLElement>("#authorization-modal");
    modal?.classList.toggle("is-visible", visible);
    modal?.setAttribute("aria-hidden", String(!visible));
  }

  private setMetric(name: string, value: number): void {
    const metric = this.root.querySelector<HTMLElement>(`[data-metric="${name}"] .metric-orbit`);
    const track = this.root.querySelector<HTMLElement>(`#${name}-track`);
    metric?.style.setProperty("--value", String(Math.max(0, Math.min(value, 100))));
    if (track) track.style.width = `${Math.max(0, Math.min(value, 100))}%`;
  }

  private drawNetwork(): void {
    const canvas = this.root.querySelector<HTMLCanvasElement>("#network-canvas");
    const context = canvas?.getContext("2d");
    if (!canvas || !context) return;
    const width = canvas.width;
    const height = canvas.height;
    context.clearRect(0, 0, width, height);
    context.strokeStyle = "rgba(76, 219, 255, .08)";
    context.lineWidth = 1;
    for (let y = 12; y < height; y += 20) {
      context.beginPath(); context.moveTo(0, y); context.lineTo(width, y); context.stroke();
    }
    const peak = Math.max(1, ...this.networkHistory.flat());
    ([0, 1] as const).forEach((channel) => {
      context.beginPath();
      context.strokeStyle = channel === 0 ? "#4cdbff" : "#6487ff";
      context.lineWidth = channel === 0 ? 2 : 1.4;
      this.networkHistory.forEach((point, index) => {
        const x = (index / Math.max(this.networkHistory.length - 1, 1)) * width;
        const y = height - 8 - (point[channel] / peak) * (height - 18);
        if (index === 0) context.moveTo(x, y); else context.lineTo(x, y);
      });
      context.stroke();
    });
  }

  private animateWaveform = (): void => {
    const canvas = this.root.querySelector<HTMLCanvasElement>("#wave-canvas");
    const context = canvas?.getContext("2d");
    if (canvas && context) {
      const activity: Record<JarvisState, number> = {
        idle: 0.08, listening: 0.85, thinking: 0.34, routing: 0.46,
        "codex-analyzing": 0.55, "codex-executing": 0.76, "n8n-executing": 0.68,
        speaking: 0.9, warning: 0.62, "authorization-required": 0.2,
        error: 0.72, offline: 0.02,
      };
      const amp = activity[this.currentState];
      const width = canvas.width;
      const height = canvas.height;
      context.clearRect(0, 0, width, height);
      const gradient = context.createLinearGradient(0, 0, width, 0);
      gradient.addColorStop(0, "rgba(76,219,255,0)");
      gradient.addColorStop(0.18, "rgba(76,219,255,.7)");
      gradient.addColorStop(0.5, "rgba(202,248,255,.95)");
      gradient.addColorStop(0.82, "rgba(76,219,255,.7)");
      gradient.addColorStop(1, "rgba(76,219,255,0)");
      context.strokeStyle = gradient;
      context.lineWidth = 1.5;
      context.beginPath();
      for (let x = 0; x <= width; x += 3) {
        const centerBias = Math.sin((x / width) * Math.PI);
        const carrier = Math.sin(x * 0.068 + this.wavePhase) * 0.55 + Math.sin(x * 0.14 - this.wavePhase * 1.7) * 0.24;
        const burst = Math.sin(x * 0.019 + this.wavePhase * 0.35) * 0.2;
        const y = height / 2 + (carrier + burst) * height * 0.38 * amp * centerBias;
        if (x === 0) context.moveTo(x, y); else context.lineTo(x, y);
      }
      context.stroke();
      context.strokeStyle = "rgba(76,219,255,.16)";
      context.beginPath(); context.moveTo(0, height / 2); context.lineTo(width, height / 2); context.stroke();
      this.text("wave-level", `${(-54 + amp * 46).toFixed(1)} dB`);
      this.wavePhase += 0.045 + amp * 0.12;
    }
    this.animationFrame = requestAnimationFrame(this.animateWaveform);
  };

  private text(id: string, value: string): void {
    const element = this.root.querySelector<HTMLElement>(`#${id}`);
    if (element && element.textContent !== value) element.textContent = value;
  }
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** index).toFixed(index > 2 ? 1 : 0)} ${units[index]}`;
}

export function formatRate(bytes: number): string {
  return `${formatBytes(bytes)}/s`;
}

export function formatDuration(seconds: number): string {
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  return `${days.toString().padStart(2, "0")}D ${hours.toString().padStart(2, "0")}:${minutes.toString().padStart(2, "0")}`;
}
