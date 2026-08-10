export const TASK_STATUSES = [
  "QUEUED", "ANALYZING", "WAITING_TOOL", "WAITING_AUTHORIZATION", "EXECUTING",
  "COMPLETED", "FAILED", "CANCELLED", "TIMEOUT",
] as const;

export type TaskStatus = typeof TASK_STATUSES[number];

export interface CreateTaskRequest {
  task_type: string;
  objective: string;
  session_id: string;
  correlation_id: string;
  target?: string;
  context?: Record<string, unknown>;
}

export interface CodexTask {
  task_id: string;
  session_id: string;
  correlation_id: string;
  status: TaskStatus;
  created_at: string;
  updated_at: string;
  request: CreateTaskRequest;
  result?: { output: string; thread_id: string };
  error?: { code: string; message: string };
}

const ID = /^[A-Za-z0-9_.:-]{1,128}$/;
const NAME = /^[a-z0-9_.:-]{1,64}$/;
const SECRET_FIELDS = /(^|_)(authorization|cookie|password|passwd|secret|token|api_key|private_key|credential)s?$/i;

export function parseCreateTask(value: unknown): CreateTaskRequest | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const input = value as Record<string, unknown>;
  const allowed = new Set(["task_type", "objective", "session_id", "correlation_id", "target", "context"]);
  if (Object.keys(input).some((key) => !allowed.has(key))) return null;
  if (typeof input.task_type !== "string" || !NAME.test(input.task_type)
    || typeof input.objective !== "string" || input.objective.trim().length === 0
    || Buffer.byteLength(input.objective) > 8_000
    || typeof input.session_id !== "string" || !ID.test(input.session_id)
    || typeof input.correlation_id !== "string" || !ID.test(input.correlation_id)
    || (input.target !== undefined && (typeof input.target !== "string" || !NAME.test(input.target)))) return null;
  const context = input.context;
  if (context !== undefined && (!context || typeof context !== "object" || Array.isArray(context))) return null;
  if (context && (Buffer.byteLength(JSON.stringify(context)) > 16_384 || containsSecretField(context))) return null;
  return {
    task_type: input.task_type,
    objective: input.objective.trim(),
    session_id: input.session_id,
    correlation_id: input.correlation_id,
    ...(input.target ? { target: input.target as string } : {}),
    ...(context ? { context: context as Record<string, unknown> } : {}),
  };
}

function containsSecretField(value: unknown, depth = 0): boolean {
  if (depth > 8) return true;
  if (Array.isArray(value)) return value.some((entry) => containsSecretField(entry, depth + 1));
  if (!value || typeof value !== "object") return false;
  return Object.entries(value).some(([key, entry]) => SECRET_FIELDS.test(key.replaceAll("-", "_")) || containsSecretField(entry, depth + 1));
}
