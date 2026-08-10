import type { CoreHealth } from "../core/types";
import { RuntimeCapabilityError, type JarvisRuntimeClient } from "./client";

const MAX_HEALTH_BYTES = 4_096;
const MAX_MESSAGE_BYTES = 4_096;

interface ComponentEnvelope {
  id: string;
  label: string;
  status: "healthy" | "degraded" | "unavailable";
  agent_status: "REALTIME" | "READY" | "BUSY" | "DEGRADED" | "ERROR" | "OFFLINE";
  version: string;
  latency_ms?: number;
  last_seen_ms?: number;
  error?: string;
}

interface HealthEnvelope {
  api_version: string;
  status: "healthy" | "degraded" | "unavailable";
  state: "IDLE" | "LISTENING" | "THINKING" | "ROUTING" | "EXECUTING" | "SPEAKING"
    | "AUTHORIZATION_REQUIRED" | "WARNING" | "ERROR" | "OFFLINE";
  components: ComponentEnvelope[];
}

export class WebRuntimeClient implements JarvisRuntimeClient {
  readonly kind = "browser" as const;
  private csrfToken: string | null = null;

  constructor(private readonly fetcher: typeof fetch = globalThis.fetch.bind(globalThis)) {}

  async coreHealth(): Promise<CoreHealth> {
    const started = performance.now();
    const response = await this.fetcher("/api/v1/health", {
      method: "GET",
      credentials: "same-origin",
      headers: { Accept: "application/json" },
      redirect: "error",
      cache: "no-store",
    });
    if (!response.ok) throw new Error("Core health check was rejected");
    const length = Number(response.headers.get("content-length") ?? "0");
    if (length > MAX_HEALTH_BYTES) throw new Error("Core health response is too large");
    const text = await response.text();
    if (new TextEncoder().encode(text).byteLength > MAX_HEALTH_BYTES) {
      throw new Error("Core health response is too large");
    }
    let value: unknown;
    try {
      value = JSON.parse(text);
    } catch {
      throw new Error("Core health response is invalid");
    }
    const health = parseSystemHealth(value);
    return { ...health, latencyMs: Math.round(performance.now() - started) };
  }

  async hasSession(): Promise<boolean> {
    const response = await this.fetcher("/api/v1/session", {
      method: "GET",
      credentials: "same-origin",
      headers: { Accept: "application/json" },
      redirect: "error",
      cache: "no-store",
    });
    if (response.status === 401) return false;
    if (!response.ok) throw new Error("Session status is unavailable");
    const body: unknown = await response.json();
    const csrfToken = (body as { csrf_token?: unknown }).csrf_token;
    const valid = Boolean(
      body && typeof body === "object" && !Array.isArray(body)
      && Object.keys(body).every((key) => ["api_version", "authenticated", "csrf_token"].includes(key))
      && (body as { api_version?: unknown }).api_version === "v1"
      && (body as { authenticated?: unknown }).authenticated === true
      && typeof csrfToken === "string"
      && /^[0-9a-f]{64}$/.test(csrfToken)
    );
    this.csrfToken = valid ? csrfToken as string : null;
    return valid;
  }

  async login(accessKey: string): Promise<void> {
    const key = accessKey.trim();
    if (key.length < 32 || key.length > 4096 || /\s/.test(key)) {
      throw new Error("Access key is invalid");
    }
    const response = await this.fetcher("/api/v1/session", {
      method: "POST",
      credentials: "same-origin",
      headers: { Authorization: `Bearer ${key}`, Accept: "application/json" },
      redirect: "error",
      cache: "no-store",
    });
    if (!response.ok) throw new Error("Authentication was rejected");
    if (!await this.hasSession()) throw new Error("Session could not be established");
  }

  async logout(): Promise<void> {
    if (!this.csrfToken && !await this.hasSession()) return;
    const response = await this.fetcher("/api/v1/session", {
      method: "DELETE",
      credentials: "same-origin",
      headers: { "x-jarvis-csrf": this.csrfToken ?? "", Accept: "application/json" },
      redirect: "error",
      cache: "no-store",
    });
    this.csrfToken = null;
    if (!response.ok) throw new Error("Logout was rejected");
  }

  websocketUrl(location: Pick<Location, "protocol" | "host"> = window.location): string {
    if (location.protocol !== "https:") throw new Error("Realtime requires HTTPS");
    return `wss://${location.host}/ws`;
  }

  voiceWebsocketUrl(location: Pick<Location, "protocol" | "host"> = window.location): string {
    if (location.protocol !== "https:") throw new Error("Voice realtime requires HTTPS");
    return `wss://${location.host}/ws/voice`;
  }

  async conversation(message: string): Promise<import("../core/types").CoreConversation> {
    const instruction = message.trim();
    if (!instruction || new TextEncoder().encode(instruction).byteLength > MAX_MESSAGE_BYTES) {
      throw new Error("Conversation message is invalid");
    }
    if (!this.csrfToken && !await this.hasSession()) {
      throw new RuntimeCapabilityError("conversation", "Authenticated browser session is required");
    }
    const requestId = crypto.randomUUID();
    const response = await this.fetcher("/api/v1/requests", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json", "x-jarvis-csrf": this.csrfToken ?? "", Accept: "application/json" },
      redirect: "error",
      cache: "no-store",
      body: JSON.stringify({ api_version: "v1", request_id: requestId, session_id: requestId, kind: "conversation", message: instruction }),
    });
    if (!response.ok) throw new Error("Core request was rejected");
    const body = await response.json() as Record<string, unknown>;
    const data = body.data as Record<string, unknown> | undefined;
    if (body.api_version !== "v1" || body.request_id !== requestId || body.status !== "completed"
      || typeof body.audit_id !== "string" || typeof data?.message !== "string" || typeof data?.mode !== "string") {
      throw new Error("Core response correlation failed");
    }
    return { requestId, status: body.status, auditId: body.audit_id, message: data.message, mode: data.mode };
  }

  async telemetry(): Promise<never> {
    throw new RuntimeCapabilityError(
      "telemetry",
      "Server telemetry is not available until the realtime gateway is connected",
    );
  }
}

export function parseSystemHealth(candidate: unknown): CoreHealth {
  const health = validateHealth(candidate);
  return {
    online: true,
    apiVersion: health.api_version,
    status: health.status,
    latencyMs: 0,
    state: health.state.toLowerCase().replace(/_/g, "-") as CoreHealth["state"],
    components: health.components.map((component) => ({
        id: component.id,
        label: component.label,
        status: component.status,
        agentStatus: component.agent_status.toLowerCase() as "realtime" | "ready" | "busy" | "degraded" | "error" | "offline",
        version: component.version,
        latencyMs: component.latency_ms,
        lastSeenMs: component.last_seen_ms,
        error: component.error,
      })),
  };
}

function validateHealth(candidate: unknown): HealthEnvelope {
  if (
    !candidate
    || typeof candidate !== "object"
    || Array.isArray(candidate)
    || Object.keys(candidate).some((key) => !["api_version", "status", "state", "components"].includes(key))
  ) {
    throw new Error("Core health response is invalid");
  }
  const health = candidate as Partial<HealthEnvelope>;
  if (
    health.api_version !== "v1"
    || !["healthy", "degraded", "unavailable"].includes(health.status ?? "")
    || !isJarvisState(health.state)
    || !Array.isArray(health.components)
    || health.components.length !== 8
    || health.components.some((component) => !isComponent(component))
  ) {
    throw new Error("Core health response is invalid");
  }
  return health as HealthEnvelope;
}

function isJarvisState(value: unknown): value is HealthEnvelope["state"] {
  return typeof value === "string" && [
    "IDLE", "LISTENING", "THINKING", "ROUTING", "EXECUTING", "SPEAKING",
    "AUTHORIZATION_REQUIRED", "WARNING", "ERROR", "OFFLINE",
  ].includes(value);
}

function isComponent(value: unknown): value is ComponentEnvelope {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const component = value as Partial<ComponentEnvelope>;
  const allowed = ["id", "label", "status", "agent_status", "version", "latency_ms", "last_seen_ms", "error"];
  return Object.keys(value).every((key) => allowed.includes(key))
    && typeof component.id === "string"
    && typeof component.label === "string"
    && ["healthy", "degraded", "unavailable"].includes(component.status ?? "")
    && ["REALTIME", "READY", "BUSY", "DEGRADED", "ERROR", "OFFLINE"].includes(component.agent_status ?? "")
    && typeof component.version === "string"
    && (component.latency_ms === undefined || Number.isSafeInteger(component.latency_ms))
    && (component.last_seen_ms === undefined || Number.isSafeInteger(component.last_seen_ms))
    && (component.error === undefined || typeof component.error === "string");
}
