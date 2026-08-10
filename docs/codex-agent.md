# Codex expert agent

`services/codex-service` is the internal adapter around the official `@openai/codex-sdk`. JARVIS Core sees task resources, not CLI output or App Server JSON-RPC.

Milestone 1 supports technical analysis only:

1. Core creates a normalized task.
2. Codex Service returns `QUEUED` immediately.
3. Core observes `ANALYZING` and terminal task state without blocking WebSocket processing.
4. The response returns through Core and may then enter TTS.

The service runs Codex with `sandboxMode: read-only` and `approvalPolicy: never`. It receives no MCP servers and no infrastructure credentials. Context fields resembling credentials are rejected before a task is created. Threads are scoped by JARVIS session and are never shared across sessions.

Production authentication uses a dedicated LiteLLM virtual key delivered as a systemd credential. The server-side adapter calls LiteLLM's OpenAI-compatible `/v1/chat/completions` endpoint using the `jarvis-technical` alias; no direct OpenAI credential is installed on the Codex host. The official Codex SDK remains the local/development adapter, while the gateway adapter avoids LiteLLM's unsupported Codex Responses WebSocket upgrade. The local ChatGPT login is permitted only for development and is not copied to a server.

Future tool use must follow `Codex -> structured request -> MCP Gateway -> schema validation -> policy -> authorization -> restricted executor -> verification -> audit`. Codex cannot select its own approval or sandbox policy.
