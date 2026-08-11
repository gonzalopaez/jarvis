# J.A.R.V.I.S

JARVIS is a security-first client-server AI system with a lightweight cinematic Web UI, an optional transitional Tauri client, a single governed Core API, modular services, and explicit trust boundaries.

## v0.1-clean

This baseline contains only:

- the approved cinematic Desktop HUD;
- real local Linux telemetry exposed by a narrow Tauri command;
- a typed in-process Event Bus;
- provider-neutral mock adapters;
- tests and architecture/security documentation.

No external backend, model provider, automation platform, voice backend, infrastructure API, credential store, shell executor, or production deployment is connected.

## Trust boundary

    Browser / transitional Tauri client -> HTTPS/WSS
      -> internal DNS -> Nginx TLS Gateway -> Jarvis Core

Jarvis Core will own all future connections to LiteLLM, n8n, Voice Service, Codex Bridge, MCP/agents and infrastructure adapters. Secrets belong in OpenBao and must never enter source control, Desktop configuration, prompts, model context, logs, or tool output.

Sensitive actions must follow:

    Structured Request -> Schema Validation -> Policy Engine -> Authorization
    -> Restricted Executor -> Credential Broker -> OpenBao -> Target
    -> Verification -> Audit

There is no direct LLM-to-shell path.

## Development

Web UI requirements: Node.js/npm.

    cd apps/desktop
    npm ci
    npm test
    npm run build
    npm run dev

The generated `dist/` is a static Web UI. It reads aggregate same-origin Core health without Tauri. After exchanging an existing operator access key for a bounded HttpOnly session, it can use authenticated commands, realtime telemetry, security alerts and voice without exposing service credentials to JavaScript.

For local browser development, an optional `JARVIS_CORE_URL` configures Vite to proxy only `/v1/health`. It is not included in the browser bundle. Protected routes are deliberately not proxied by the development server.

The transitional Tauri client additionally requires Rust and native Tauri dependencies:

    cd apps/desktop
    cargo test --manifest-path src-tauri/Cargo.toml
    npm run tauri dev

See [architecture](docs/architecture.md), [security policy](SECURITY.md), and [roadmap](docs/roadmap.md).

Operational observability is normalized by the server-side [Telemetry Service](docs/telemetry.md); browsers never query Prometheus or Wazuh directly.
