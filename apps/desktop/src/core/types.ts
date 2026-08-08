export type JarvisState =
  | "idle"
  | "listening"
  | "thinking"
  | "routing"
  | "codex-analyzing"
  | "codex-executing"
  | "n8n-executing"
  | "speaking"
  | "warning"
  | "authorization-required"
  | "error"
  | "offline";

export type EventSeverity = "info" | "success" | "warning" | "error";
export type ServiceState = "ready" | "active" | "staged" | "offline" | "warning";

export interface TelemetrySnapshot {
  timestampMs: number;
  cpuUsage: number;
  memoryUsed: number;
  memoryTotal: number;
  memoryUsage: number;
  diskUsed: number;
  diskTotal: number;
  diskUsage: number;
  networkRxPerSec: number;
  networkTxPerSec: number;
  uptimeSeconds: number;
  loadAverage: [number, number, number];
  hostname: string;
  kernel: string;
}

export interface AgentStatus {
  id: string;
  label: string;
  state: ServiceState;
  detail: string;
  latencyMs?: number;
  simulated: boolean;
}

export interface ActivityEvent {
  id: string;
  timestamp: Date;
  component: string;
  event: string;
  severity: EventSeverity;
  simulated?: boolean;
}

export interface JarvisModel {
  state: JarvisState;
  telemetry: TelemetrySnapshot | null;
  telemetryOnline: boolean;
  agents: AgentStatus[];
  activity: ActivityEvent[];
  userTranscript: string;
  jarvisTranscript: string;
  developerControls: boolean;
}

export interface AppEvents {
  "state.changed": { state: JarvisState; previous: JarvisState };
  "telemetry.updated": TelemetrySnapshot;
  "telemetry.failed": { message: string };
  "activity.created": ActivityEvent;
  "transcript.updated": { role: "user" | "jarvis"; text: string };
  "authorization.approved": { action: string };
  "authorization.denied": { action: string };
}

export const STATE_LABELS: Record<JarvisState, string> = {
  idle: "SYSTEM // STANDBY",
  listening: "VOICE // LISTENING",
  thinking: "JARVIS // THINKING",
  routing: "INTENT // ROUTING",
  "codex-analyzing": "CODEX // ANALYZING",
  "codex-executing": "CODEX // EXECUTING",
  "n8n-executing": "N8N // EXECUTING",
  speaking: "JARVIS // SPEAKING",
  warning: "SYSTEM // WARNING",
  "authorization-required": "AUTHORIZATION // REQUIRED",
  error: "SYSTEM // ERROR",
  offline: "SYSTEM // OFFLINE",
};
