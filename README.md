# J.A.R.V.I.S

JARVIS is a security-first client-server AI system with a lightweight cinematic Web UI, an optional transitional Tauri client, a single governed Core API, modular services, and explicit trust boundaries.

## Current baseline

`origin/main@a2f37e0` contains:

- the approved cinematic Desktop HUD;
- real local Linux telemetry exposed by a narrow Tauri command;
- a typed in-process Event Bus;
- the governed Core API, private Voice and MCP service contracts;
- LiteLLM model aliases and bounded Codex routing;
- a tiered `PolicyEngine` with proposal-only Wazuh and Proxmox agents;
- parallel cross-domain evidence collection;
- tests and architecture/security documentation.

Production verification and test-only implementation are intentionally kept
separate in [STATUS.md](STATUS.md). Write execution remains disabled.

## Trust boundary

    Browser / transitional Tauri client -> HTTPS/WSS
      -> internal DNS -> Nginx TLS Gateway -> Jarvis Core

Jarvis Core owns the governed boundary to LiteLLM, Voice, Codex and domain
agents. n8n performs mechanical correlation and notification; domain agents
may submit proposals but cannot authorize themselves. Secrets belong in
OpenBao and must never enter source control, Desktop configuration, prompts,
model context, logs, or tool output.

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
