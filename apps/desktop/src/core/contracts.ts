export type TaskRisk = "read-only" | "modification" | "critical";

export interface AgentTaskRequest {
  taskId: string;
  instruction: string;
  context?: Record<string, unknown>;
  requestedRisk: TaskRisk;
}

export type AgentTaskEvent =
  | { type: "task.accepted"; taskId: string }
  | { type: "task.analyzing"; taskId: string; summary?: string }
  | { type: "task.authorization-required"; taskId: string; action: string; impact: string; risk: TaskRisk }
  | { type: "task.executing"; taskId: string; action?: string }
  | { type: "task.completed"; taskId: string; result: string }
  | { type: "task.failed"; taskId: string; error: string };

/**
 * Provider-neutral boundary for v0.2.
 * A future Codex/LiteLLM implementation must live behind this interface;
 * UI modules must never import provider SDKs or credentials.
 */
export interface AgentAdapter {
  readonly id: string;
  readonly displayName: string;
  health(): Promise<{ online: boolean; latencyMs?: number; detail?: string }>;
  execute(task: AgentTaskRequest, emit: (event: AgentTaskEvent) => void): Promise<void>;
  cancel(taskId: string): Promise<boolean>;
}
