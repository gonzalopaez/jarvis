# JARVIS Codex Service

Internal task-oriented expert adapter. It is never exposed to the browser.

Milestone 1 security posture:

- Codex runs with `read-only` sandbox and `approvalPolicy: never`.
- No MCP servers or infrastructure tools are provided.
- Requests reject secret-shaped context fields and unknown fields.
- Authentication uses a bounded service token file; no token belongs in environment variables or Git.
- Production authentication is read from `JARVIS_CODEX_GATEWAY_TOKEN_FILE` and sent only to the configured LiteLLM base URL; local development may use an already authenticated Codex profile.
- In production the gateway adapter uses LiteLLM's OpenAI-compatible Chat Completions API. This avoids the Codex Responses WebSocket upgrade, which LiteLLM does not expose, while preserving the gateway as the sole provider/authentication boundary.
- Responses never assert that an infrastructure action was executed.

The service starts tasks asynchronously with `POST /v1/tasks` and exposes bounded task status through `GET /v1/tasks/:id`.
