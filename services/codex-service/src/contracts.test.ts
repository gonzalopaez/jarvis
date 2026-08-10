import assert from "node:assert/strict";
import test from "node:test";
import { parseCreateTask } from "./contracts.js";

test("accepts a normalized technical task", () => {
  assert.ok(parseCreateTask({ task_type: "technical_diagnostic", objective: "Analyze this Rust error", session_id: "session-1", correlation_id: "request-1", context: { platform: "linux" } }));
});

test("rejects secret-shaped context and unknown fields", () => {
  assert.equal(parseCreateTask({ task_type: "technical_diagnostic", objective: "x", session_id: "s", correlation_id: "r", context: { api_key: "value" } }), null);
  assert.equal(parseCreateTask({ task_type: "technical_diagnostic", objective: "x", session_id: "s", correlation_id: "r", unexpected: true }), null);
});
