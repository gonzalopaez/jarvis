import type { JarvisState } from "../core/types";
import { JarvisStore } from "../core/state";
import type { AgentAdapter, AgentTaskEvent, AgentTaskRequest } from "../core/contracts";

export class MockAgentAdapter {
  constructor(private readonly store: JarvisStore) {}

  trigger(state: JarvisState, context?: string): void {
    this.store.setState(state, true, context);
    if (["listening", "thinking", "routing", "executing", "speaking"].includes(state)) {
      this.store.setDevAudioLevel(state === "speaking" ? .82 : state === "listening" ? .68 : .3);
    } else {
      this.store.clearAudioLevel();
    }
    const messages: Partial<Record<JarvisState, [string, string]>> = {
      listening: ["Listening for operator input.", "Voice channel open."],
      thinking: ["Analyze current system conditions.", "Processing available context."],
      routing: ["Route request to the appropriate module.", "Intent classified and routed."],
      executing: ["Apply the approved change.", "Simulated execution active."],
      speaking: ["Report the result.", "All systems responding within expected parameters."],
      warning: ["Review the active warning.", "Operator attention is advised."],
      "authorization-required": ["Authorize a protected operation.", "Execution paused pending operator approval."],
      error: ["Resolve the active fault.", "A simulated subsystem fault was detected."],
      offline: ["Restore subsystem connectivity.", "External service links are offline."],
    };
    const message = messages[state];
    if (message) {
      this.store.setTranscript("user", message[0]);
      this.store.setTranscript("jarvis", message[1]);
    }
  }
}

/** Provider-neutral v0.1 mock used by future integration tests. */
export class MockCodexAdapter implements AgentAdapter {
  readonly id = "codex-mock";
  readonly displayName = "Codex Bridge Mock";

  async health(): Promise<{ online: boolean; detail: string }> {
    return { online: true, detail: "SIMULATED ADAPTER" };
  }

  async execute(task: AgentTaskRequest, emit: (event: AgentTaskEvent) => void): Promise<void> {
    emit({ type: "task.accepted", taskId: task.taskId });
    emit({ type: "task.analyzing", taskId: task.taskId, summary: "Simulated analysis" });
    emit({ type: "task.completed", taskId: task.taskId, result: "No provider was contacted." });
  }

  async cancel(): Promise<boolean> {
    return true;
  }
}
