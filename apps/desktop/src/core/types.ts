import type { AudioVisualizerReading } from "../audio/source";

export type JarvisState =
  | "idle"
  | "listening"
  | "thinking"
  | "routing"
  | "executing"
  | "speaking"
  | "warning"
  | "authorization-required"
  | "error"
  | "offline";

export type EventSeverity = "info" | "success" | "warning" | "error";
export type ServiceState = "realtime" | "ready" | "busy" | "degraded" | "error" | "offline";

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
  diskReadBytesPerSec?: number;
  diskWriteBytesPerSec?: number;
  temperatures?: Array<{ sensor: string; celsius: number }>;
}

export interface AgentStatus {
  id: string;
  label: string;
  state: ServiceState;
  detail: string;
  latencyMs?: number;
  simulated: boolean;
}

export interface CoreHealth {
  online: boolean;
  apiVersion: string;
  status: string;
  latencyMs: number;
  state?: JarvisState;
  components?: ComponentHealth[];
}

export interface ComponentHealth {
  id: string;
  label: string;
  status: "healthy" | "degraded" | "unavailable";
  agentStatus: ServiceState;
  version: string;
  latencyMs?: number;
  lastSeenMs?: number;
  error?: string;
}

export type SecuritySeverity = "low" | "medium" | "high" | "critical";

export interface SecurityAlert {
  id: string;
  host?: string;
  timestampMs: number;
  severity: SecuritySeverity;
  title: string;
  description: string;
}

export interface SecurityTelemetrySnapshot {
  timestampMs: number;
  source: "wazuh" | "jarvis";
  failedLogins?: number;
  sudoCommands?: number;
  fimChanges?: number;
  newProcesses?: number;
  networkConnections?: number;
  inboundConnections?: number;
  outboundConnections?: number;
  privilegedUsersOnline?: number;
  listeningPorts?: number;
}

export interface CoreConversation {
  requestId: string;
  status: string;
  auditId: string;
  message: string;
  mode: string;
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
  audioVisualization: AudioVisualizerReading;
  operationContext?: string;
  securityTelemetry: SecurityTelemetrySnapshot | null;
  securityAlerts: SecurityAlert[];
}

export interface AppEvents {
  "state.changed": { state: JarvisState; previous: JarvisState };
  "telemetry.updated": TelemetrySnapshot;
  "telemetry.failed": { message: string };
  "telemetry.unavailable": { message: string };
  "core.health.updated": CoreHealth;
  "core.health.failed": { message: string };
  "activity.created": ActivityEvent;
  "transcript.updated": { role: "user" | "jarvis"; text: string };
  "authorization.approved": { action: string };
  "authorization.denied": { action: string };
  "realtime.connected": { connectedAtMs: number };
  "realtime.disconnected": { reason: string };
  "realtime.unavailable": { reason: string };
  "realtime.resync.required": Record<string, never>;
  "realtime.state.changed": { state: JarvisState };
  "realtime.agent.changed": ComponentHealth;
  "realtime.activity": { component: string; message: string; severity: EventSeverity };
  "voice.input.level": { level: number };
  "voice.output.level": { level: number };
  "security.telemetry.updated": SecurityTelemetrySnapshot;
  "security.alert": SecurityAlert;
}
