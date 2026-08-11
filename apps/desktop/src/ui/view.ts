import type { JarvisModel, JarvisState, TelemetrySnapshot } from "../core/types";
import { STATE_ANIMATION, STATE_PRESENTATION } from "../core/state-theme";

export class JarvisView {
  private readonly networkHistory: Array<[number, number]> = [];
  private readonly metricHistory: Record<"cpu" | "memory" | "disk", number[]> = { cpu: [], memory: [], disk: [] };
  private wavePhase = 0;
  private animationFrame = 0;
  private animationTimer = 0;
  private currentState: JarvisState = "idle";
  private agentsSignature = "";
  private activitySignature = "";
  private securitySignature = "";
  private interactionSignature = "";
  private audioLevel = 0;
  private audioSource: JarvisModel["audioVisualization"]["source"] = "none";

  constructor(private readonly root: HTMLElement) {
    document.addEventListener("visibilitychange", this.handleVisibilityChange);
    this.scheduleWaveform(true);
  }

  render(model: JarvisModel): void {
    this.currentState = model.state;
    const presentation = STATE_PRESENTATION[model.state];
    this.root.dataset.state = model.state;
    this.root.dataset.theme = presentation.theme;
    this.root.classList.toggle("is-active-state", presentation.animateWhenIdle);
    this.root.classList.toggle("has-core-motion", model.state !== "offline");
    this.applyAnimationProfile(model.state);
    this.text("state-label", model.operationContext ? model.operationContext.replace(/ /g, " // ") : presentation.label);
    this.text("wave-mode", presentation.mode);
    this.audioLevel = model.audioVisualization.level;
    this.audioSource = model.audioVisualization.source;
    this.text("voice-state", model.state.toUpperCase().replace(/-/g, " "));
    this.text("voice-context", voiceContext(model.state, this.audioSource));
    this.root.dataset.audioSource = this.audioSource;
    this.text("user-transcript", model.userTranscript);
    this.text("jarvis-transcript", model.jarvisTranscript);
    this.renderAgents(model);
    this.renderActivity(model);
    this.renderCoreInteraction(model);
    this.renderSecurity(model);
    this.renderAuthorization(model.state === "authorization-required");
    document.querySelector("#dev-controls")?.classList.toggle("is-hidden", !model.developerControls);
    if (model.telemetry) this.renderTelemetry(model.telemetry);
    this.scheduleWaveform();
  }

  private applyAnimationProfile(state: JarvisState): void {
    const profile = STATE_ANIMATION[state];
    const values: Record<string, string> = {
      "--pulse-speed": `${profile.pulseSeconds}s`,
      "--inner-speed": `${profile.innerRotationSeconds}s`,
      "--processing-speed": `${profile.processingRotationSeconds}s`,
      "--routing-speed": `${profile.routingRotationSeconds}s`,
      "--execution-speed": `${profile.executionRotationSeconds}s`,
      "--telemetry-speed": `${profile.telemetryRotationSeconds}s`,
      "--outer-speed": `${profile.outerRotationSeconds}s`,
      "--sweep-speed": `${profile.sweepSeconds}s`,
      "--core-glow": String(profile.glowIntensity),
      "--core-glow-radius": `${Math.round(10 + profile.glowIntensity * 24)}px`,
      "--core-heart-scale": String(1 + profile.glowIntensity * .035),
      "--core-heart-brightness": String(1 + profile.glowIntensity * .18),
      "--routing-activity": String(profile.routingActivity),
      "--execution-activity": String(profile.executionActivity),
      "--orbital-activity": String(profile.orbitalActivity),
      "--routing-opacity": String(.12 + profile.routingActivity * .72),
      "--execution-opacity": String(.1 + profile.executionActivity * .72),
      "--orbit-one-opacity": String(.25 + profile.orbitalActivity * .75),
      "--orbit-two-opacity": String(.2 + profile.orbitalActivity * .8),
    };
    for (const [name, value] of Object.entries(values)) this.root.style.setProperty(name, value);
  }

  destroy(): void {
    cancelAnimationFrame(this.animationFrame);
    window.clearTimeout(this.animationTimer);
    document.removeEventListener("visibilitychange", this.handleVisibilityChange);
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
    this.pushMetric("cpu", data.cpuUsage);
    this.pushMetric("memory", data.memoryUsage);
    this.pushMetric("disk", data.diskUsage);
    this.text("security-disk-read", data.diskReadBytesPerSec === undefined ? "--" : formatRate(data.diskReadBytesPerSec));
    this.text("security-disk-write", data.diskWriteBytesPerSec === undefined ? "--" : formatRate(data.diskWriteBytesPerSec));
    this.networkHistory.push([data.networkRxPerSec, data.networkTxPerSec]);
    if (this.networkHistory.length > 64) this.networkHistory.shift();
    this.drawNetwork();
  }

  private pushMetric(name: "cpu" | "memory" | "disk", value: number): void {
    const history = this.metricHistory[name];
    history.push(Math.max(0, Math.min(value, 100)));
    if (history.length > 48) history.shift();
    const canvas = this.root.querySelector<HTMLCanvasElement>(`#${name}-sparkline`);
    const context = canvas?.getContext("2d");
    if (!canvas || !context) return;
    context.clearRect(0, 0, canvas.width, canvas.height);
    context.beginPath();
    context.strokeStyle = "#4cdbff";
    context.lineWidth = 1.25;
    history.forEach((point, index) => {
      const x = index / Math.max(1, history.length - 1) * canvas.width;
      const y = canvas.height - 2 - point / 100 * (canvas.height - 4);
      if (index === 0) context.moveTo(x, y); else context.lineTo(x, y);
    });
    context.stroke();
  }

  private renderSecurity(model: JarvisModel): void {
    const signature = JSON.stringify([model.securityTelemetry, model.securityAlerts.map((alert) => alert.id)]);
    if (signature === this.securitySignature) return;
    this.securitySignature = signature;
    const telemetry = model.securityTelemetry;
    this.root.classList.toggle("has-security-data", telemetry !== null);
    this.root.classList.toggle("has-security-alerts", model.securityAlerts.length > 0);
    this.text("security-source-state", telemetry ? `${telemetry.source.toUpperCase()} / LIVE` : "OFFLINE");
    const values: Array<[string, number | undefined]> = [
      ["security-failed-logins", telemetry?.failedLogins], ["security-sudo", telemetry?.sudoCommands],
      ["security-fim", telemetry?.fimChanges], ["security-processes", telemetry?.newProcesses],
      ["security-connections", telemetry?.networkConnections], ["security-ports", telemetry?.listeningPorts],
      ["security-inbound", telemetry?.inboundConnections], ["security-outbound", telemetry?.outboundConnections],
      ["security-users", telemetry?.privilegedUsersOnline],
    ];
    for (const [id, value] of values) this.text(id, value === undefined ? "--" : String(value));
    this.text("security-alert-count", model.securityAlerts.length ? String(model.securityAlerts.length) : "--");
    const empty = this.root.querySelector<HTMLElement>("#security-alerts-empty");
    empty?.toggleAttribute("hidden", model.securityAlerts.length > 0);
    const list = this.root.querySelector<HTMLElement>("#security-alert-list");
    if (!list) return;
    list.replaceChildren(...model.securityAlerts.slice(0, 6).map((alert) => {
      const row = document.createElement("article");
      row.className = `security-alert severity-${alert.severity}`;
      const time = document.createElement("time");
      const title = document.createElement("strong");
      const description = document.createElement("p");
      time.textContent = new Date(alert.timestampMs).toLocaleTimeString("es-AR", { hour12: false });
      title.textContent = `${alert.severity.toUpperCase()} // ${alert.host ? `${alert.host} // ` : ""}${alert.title}`;
      description.textContent = alert.description;
      row.append(time, title, description);
      return row;
    }));
  }

  private renderCoreInteraction(model: JarvisModel): void {
    const signature = model.activity.slice(0, 4).map((event) => event.id).join("|");
    if (signature === this.interactionSignature) return;
    this.interactionSignature = signature;
    const stream = this.root.querySelector<HTMLElement>("#core-interaction-stream");
    if (!stream) return;
    stream.replaceChildren(...model.activity.slice(0, 4).map((event) => {
      const row = document.createElement("div");
      const time = document.createElement("time");
      const source = document.createElement("strong");
      const message = document.createElement("span");
      time.textContent = event.timestamp.toLocaleTimeString("es-AR", { hour12: false });
      source.textContent = event.component;
      message.textContent = event.event;
      row.append(time, source, message);
      return row;
    }));
  }

  private renderAgents(model: JarvisModel): void {
    const signature = JSON.stringify(model.agents);
    if (signature === this.agentsSignature) return;
    this.agentsSignature = signature;
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
    const signature = model.activity.slice(0, 12).map((event) => event.id).join("|");
    if (signature === this.activitySignature) return;
    this.activitySignature = signature;
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

  private drawWaveform(): void {
    const canvas = this.root.querySelector<HTMLCanvasElement>("#wave-canvas");
    const context = canvas?.getContext("2d");
    if (canvas && context) {
      const activity = STATE_PRESENTATION[this.currentState].waveformActivity;
      const amp = this.audioSource === "none" ? 0 : Math.max(.04, activity * this.audioLevel);
      const width = canvas.width;
      const height = canvas.height;
      context.clearRect(0, 0, width, height);
      const gradient = context.createLinearGradient(0, 0, width, 0);
      const stateColor = getComputedStyle(this.root).getPropertyValue("--state").trim() || "#4cdbff";
      gradient.addColorStop(0, "transparent");
      gradient.addColorStop(0.18, stateColor);
      gradient.addColorStop(0.5, stateColor);
      gradient.addColorStop(0.82, stateColor);
      gradient.addColorStop(1, "transparent");
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
      this.text("wave-level", this.audioSource === "none" ? "-- dB" : `${(-54 + amp * 46).toFixed(1)} dB`);
      this.wavePhase += 0.045 + amp * 0.12;
    }
  }

  private scheduleWaveform(force = false): void {
    cancelAnimationFrame(this.animationFrame);
    window.clearTimeout(this.animationTimer);
    this.drawWaveform();
    const active = this.audioSource !== "none" || this.currentState === "thinking";
    if ((!active && !force) || document.hidden) return;
    this.animationTimer = window.setTimeout(() => {
      this.animationFrame = requestAnimationFrame(() => this.scheduleWaveform());
    }, this.audioSource !== "none" ? 33 : active ? 66 : 1_000);
  }

  private handleVisibilityChange = (): void => {
    this.root.classList.toggle("is-background", document.hidden);
    this.scheduleWaveform(true);
  };

  private text(id: string, value: string): void {
    const element = this.root.querySelector<HTMLElement>(`#${id}`);
    if (element && element.textContent !== value) element.textContent = value;
  }
}

function voiceContext(state: JarvisState, source: JarvisModel["audioVisualization"]["source"]): string {
  if (state === "listening") return source === "microphone" ? "Microphone input active." : "Listening channel active; audio source unavailable.";
  if (state === "speaking") return source === "tts" ? "Synthesized voice output active." : source === "dev" ? "DEV audio visualization active." : "Voice output state active; audio source unavailable.";
  if (state === "thinking") return "Processing available context.";
  if (state === "authorization-required") return "Execution paused pending operator approval.";
  return "Awaiting authenticated voice stream.";
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
