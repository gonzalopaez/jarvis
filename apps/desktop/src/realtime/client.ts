import type { EventBus } from "../core/event-bus";
import type { AppEvents, ComponentHealth, JarvisState, TelemetrySnapshot, SecurityAlert, SecurityTelemetrySnapshot } from "../core/types";
import type { JarvisRuntimeClient } from "../runtime/client";
import { parseSystemHealth } from "../runtime/web-client";

const MAX_EVENT_BYTES = 64 * 1024;
const MAX_BACKOFF_MS = 30_000;

interface RealtimeEnvelope {
  event_version: "v1";
  event_id: string;
  type: string;
  timestamp_ms: number;
  correlation_id?: string;
  payload: unknown;
}

interface SocketLike {
  readyState: number;
  onopen: ((event: Event) => void) | null;
  onmessage: ((event: MessageEvent) => void) | null;
  onclose: ((event: CloseEvent) => void) | null;
  onerror: ((event: Event) => void) | null;
  close(code?: number, reason?: string): void;
}

type SocketFactory = (url: string) => SocketLike;

export class RealtimeClient {
  private socket: SocketLike | null = null;
  private reconnectTimer = 0;
  private attempt = 0;
  private running = false;

  constructor(
    private readonly bus: EventBus<AppEvents>,
    private readonly runtime: JarvisRuntimeClient,
    private readonly socketFactory: SocketFactory = (url) => new WebSocket(url),
    private readonly random: () => number = Math.random,
  ) {}

  async start(): Promise<void> {
    if (this.runtime.kind !== "browser" || this.socket) return;
    if (!this.running) {
      this.running = true;
      document.addEventListener("visibilitychange", this.handleVisibility);
    }
    try {
      if (!await this.runtime.hasSession()) {
        this.bus.emit("realtime.unavailable", { reason: "AUTHENTICATED_SESSION_REQUIRED" });
        return;
      }
      this.connect();
    } catch {
      this.bus.emit("realtime.unavailable", { reason: "SESSION_STATUS_UNAVAILABLE" });
    }
  }

  stop(): void {
    this.running = false;
    window.clearTimeout(this.reconnectTimer);
    document.removeEventListener("visibilitychange", this.handleVisibility);
    this.socket?.close(1000, "client shutdown");
    this.socket = null;
  }

  private connect(): void {
    if (!this.running || document.hidden || this.socket) return;
    let socket: SocketLike;
    try {
      socket = this.socketFactory(this.runtime.websocketUrl());
    } catch {
      this.scheduleReconnect();
      return;
    }
    this.socket = socket;
    socket.onopen = () => {
      this.attempt = 0;
      this.bus.emit("realtime.connected", { connectedAtMs: Date.now() });
    };
    socket.onmessage = (message) => this.handleMessage(message.data);
    socket.onerror = () => socket.close();
    socket.onclose = () => {
      if (this.socket === socket) this.socket = null;
      this.bus.emit("realtime.disconnected", { reason: "CONNECTION_CLOSED" });
      this.scheduleReconnect();
    };
  }

  private handleMessage(data: unknown): void {
    if (typeof data !== "string" || new TextEncoder().encode(data).byteLength > MAX_EVENT_BYTES) {
      this.socket?.close(1009, "event too large");
      return;
    }
    let value: unknown;
    try {
      value = JSON.parse(data);
    } catch {
      this.socket?.close(1007, "invalid event");
      return;
    }
    const event = parseEnvelope(value);
    if (!event) {
      this.socket?.close(1007, "invalid envelope");
      return;
    }
    if (event.type === "system.snapshot") {
      try {
        this.bus.emit("core.health.updated", parseSystemHealth(event.payload));
      } catch {
        this.socket?.close(1007, "invalid snapshot");
      }
    } else if (event.type === "jarvis.state.changed") {
      const state = parseState(event.payload);
      if (state) this.bus.emit("realtime.state.changed", { state });
    } else if (event.type === "agent.status.changed") {
      const agent = parseAgent(event.payload);
      if (agent) this.bus.emit("realtime.agent.changed", agent);
    } else if (event.type === "system.resync_required") {
      this.bus.emit("realtime.resync.required", {});
    } else if (event.type === "telemetry.snapshot") {
      const telemetry = parseTelemetry(event.payload);
      if (telemetry) this.bus.emit("telemetry.updated", telemetry);
    } else if (event.type === "telemetry.source.status") {
      const source = parseSourceStatus(event.payload);
      if (source) this.bus.emit("realtime.agent.changed", source);
    } else if (event.type === "security.telemetry.updated") {
      const telemetry = parseSecurityTelemetry(event.payload);
      if (telemetry) this.bus.emit("security.telemetry.updated", telemetry);
    } else if (event.type === "security.alert") {
      const alert = parseSecurityAlert(event.payload);
      if (alert) this.bus.emit("security.alert", alert);
    } else if (event.type === "router.decision") {
      const route = stringField(event.payload, "route");
      if (route) this.bus.emit("realtime.activity", { component: "ROUTER", message: `ROUTE SELECTED: ${route}`, severity: "info" });
    } else if (event.type.startsWith("codex.task.")) {
      const phase = event.type.slice("codex.task.".length).toUpperCase();
      this.bus.emit("realtime.activity", { component: "CODEX", message: `TASK ${phase}`, severity: phase === "FAILED" || phase === "TIMEOUT" ? "error" : phase === "COMPLETED" ? "success" : "info" });
    } else if (event.type.startsWith("mcp.tool.")) {
      const phase = event.type.slice("mcp.tool.".length).toUpperCase();
      this.bus.emit("realtime.activity", { component: "MCP", message: `TOOL ${phase}`, severity: phase === "FAILED" ? "error" : "info" });
    }
  }

  private scheduleReconnect(): void {
    if (!this.running || document.hidden || this.reconnectTimer !== 0) return;
    const base = Math.min(1_000 * 2 ** this.attempt, MAX_BACKOFF_MS);
    const jitter = Math.round(base * 0.2 * this.random());
    this.attempt += 1;
    this.reconnectTimer = window.setTimeout(() => {
      this.reconnectTimer = 0;
      this.connect();
    }, base + jitter);
  }

  private handleVisibility = (): void => {
    if (document.hidden) {
      this.socket?.close(1000, "background");
    } else if (this.running) {
      this.connect();
    }
  };
}

function parseSecurityTelemetry(payload: unknown): SecurityTelemetrySnapshot | null {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return null;
  const value = payload as Record<string, unknown>;
  if (typeof value.timestamp_ms !== "number" || !Number.isFinite(value.timestamp_ms)) return null;
  return {
    timestampMs: value.timestamp_ms,
    source: "wazuh",
    failedLogins: numberField(value, "failed_logins"),
    sudoCommands: numberField(value, "sudo_commands"),
    newProcesses: numberField(value, "new_processes"),
    fimChanges: numberField(value, "fim_changes"),
    networkConnections: numberField(value, "network_connections"),
    inboundConnections: numberField(value, "inbound_connections"),
    outboundConnections: numberField(value, "outbound_connections"),
    privilegedUsersOnline: numberField(value, "privileged_users_online"),
    listeningPorts: numberField(value, "listening_ports"),
  };
}

function parseSecurityAlert(payload: unknown): SecurityAlert | null {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return null;
  const value = payload as Record<string, unknown>;
  if (typeof value.id !== "string" || typeof value.timestamp_ms !== "number"
    || typeof value.severity !== "string" || typeof value.title !== "string"
    || typeof value.description !== "string") return null;
  const severity = value.severity.toLowerCase() as SecurityAlert["severity"];
  if (!["low", "medium", "high", "critical"].includes(severity)) return null;
  return { id: value.id, host: typeof value.host === "string" ? value.host.slice(0, 128) : undefined, timestampMs: value.timestamp_ms, severity, title: value.title.slice(0, 160), description: value.description.slice(0, 500) };
}

function numberField(value: Record<string, unknown>, key: string): number | undefined {
  const result = value[key];
  return typeof result === "number" && Number.isFinite(result) && result >= 0 ? result : undefined;
}

function parseTelemetry(payload: unknown): TelemetrySnapshot | null {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return null;
  const value = payload as Record<string, unknown>;
  const requiredNumbers = [
    "timestamp_ms", "cpu_usage_percent", "memory_used_bytes", "memory_total_bytes",
    "filesystem_used_bytes", "filesystem_total_bytes", "disk_read_bytes_per_second",
    "disk_write_bytes_per_second", "network_receive_bytes_per_second",
    "network_transmit_bytes_per_second", "uptime_seconds",
  ];
  if (typeof value.host !== "string" || typeof value.kernel !== "string"
    || requiredNumbers.some((key) => typeof value[key] !== "number" || !Number.isFinite(value[key]))
    || !Array.isArray(value.load_average) || value.load_average.length !== 3
    || value.load_average.some((item) => typeof item !== "number" || !Number.isFinite(item))
    || !Array.isArray(value.temperatures)) return null;
  const memoryTotal = value.memory_total_bytes as number;
  const diskTotal = value.filesystem_total_bytes as number;
  if (memoryTotal <= 0 || diskTotal <= 0) return null;
  return {
    timestampMs: value.timestamp_ms as number,
    cpuUsage: value.cpu_usage_percent as number,
    memoryUsed: value.memory_used_bytes as number,
    memoryTotal,
    memoryUsage: (value.memory_used_bytes as number) / memoryTotal * 100,
    diskUsed: value.filesystem_used_bytes as number,
    diskTotal,
    diskUsage: (value.filesystem_used_bytes as number) / diskTotal * 100,
    networkRxPerSec: value.network_receive_bytes_per_second as number,
    networkTxPerSec: value.network_transmit_bytes_per_second as number,
    uptimeSeconds: value.uptime_seconds as number,
    loadAverage: value.load_average as [number, number, number],
    hostname: value.host,
    kernel: value.kernel,
    diskReadBytesPerSec: value.disk_read_bytes_per_second as number,
    diskWriteBytesPerSec: value.disk_write_bytes_per_second as number,
    temperatures: value.temperatures as Array<{ sensor: string; celsius: number }>,
  };
}

function parseSourceStatus(payload: unknown): ComponentHealth | null {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return null;
  const source = (payload as { source?: unknown }).source;
  const status = (payload as { status?: unknown }).status;
  if (typeof source !== "string" || typeof status !== "string") return null;
  const id = source === "prometheus" ? "monitor" : source === "wazuh" ? "security" : null;
  if (!id) return null;
  const healthy = status === "healthy";
  return {
    id,
    label: id === "monitor" ? "SYSTEM MONITOR" : "SECURITY AGENT",
    status: healthy ? "healthy" : "unavailable",
    agentStatus: healthy ? "realtime" : "offline",
    version: "adapter",
    error: healthy ? undefined : status,
  };
}

function parseEnvelope(value: unknown): RealtimeEnvelope | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const candidate = value as Partial<RealtimeEnvelope>;
  const allowed = ["event_version", "event_id", "type", "timestamp_ms", "correlation_id", "payload"];
  if (!Object.keys(value).every((key) => allowed.includes(key))) return null;
  if (candidate.event_version !== "v1"
    || typeof candidate.event_id !== "string"
    || !/^event-[0-9a-f]{16}$/.test(candidate.event_id)
    || typeof candidate.type !== "string"
    || !Number.isSafeInteger(candidate.timestamp_ms)
    || candidate.payload === undefined
    || (candidate.correlation_id !== undefined && typeof candidate.correlation_id !== "string")) {
    return null;
  }
  return candidate as RealtimeEnvelope;
}

function parseState(payload: unknown): JarvisState | null {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return null;
  const state = (payload as { state?: unknown }).state;
  if (typeof state !== "string") return null;
  const normalized = state.toLowerCase().replace(/_/g, "-") as JarvisState;
  return ["idle", "listening", "thinking", "routing", "executing", "speaking", "authorization-required", "warning", "error", "offline"].includes(normalized)
    ? normalized : null;
}

function parseAgent(payload: unknown): ComponentHealth | null {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return null;
  const value = payload as Record<string, unknown>;
  const status = typeof value.status === "string" ? value.status.toLowerCase() : "";
  const rawAgentStatus = value.agent_status ?? value.agentStatus;
  const agentStatus = typeof rawAgentStatus === "string" ? rawAgentStatus.toLowerCase() : "";
  if (typeof value.id !== "string" || typeof value.label !== "string"
    || !["healthy", "degraded", "unavailable"].includes(status)
    || !["realtime", "ready", "busy", "degraded", "error", "offline"].includes(agentStatus)
    || typeof value.version !== "string") return null;
  return {
    id: value.id,
    label: value.label,
    status: status as ComponentHealth["status"],
    agentStatus: agentStatus as ComponentHealth["agentStatus"],
    version: value.version,
    ...(typeof value.latency_ms === "number" ? { latencyMs: value.latency_ms } : {}),
    ...(typeof value.last_seen_ms === "number" ? { lastSeenMs: value.last_seen_ms } : {}),
    ...(typeof value.error === "string" ? { error: value.error } : {}),
  };
}

function stringField(payload: unknown, key: string): string | null {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return null;
  const value = (payload as Record<string, unknown>)[key];
  return typeof value === "string" && value.length <= 128 ? value : null;
}
