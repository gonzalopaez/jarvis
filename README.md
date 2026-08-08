# J.A.R.V.I.S

JARVIS is a security-first local AI system with a cinematic Tauri Desktop, a single governed Core API, modular services, and explicit trust boundaries.

## v0.1-clean

This baseline contains only:

- the approved cinematic Desktop HUD;
- real local Linux telemetry exposed by a narrow Tauri command;
- a typed in-process Event Bus;
- provider-neutral mock adapters;
- tests and architecture/security documentation.

No external backend, model provider, automation platform, voice backend, infrastructure API, credential store, shell executor, or production deployment is connected.

## Trust boundary

    JARVIS Desktop -> HTTPS/WSS -> internal DNS -> Nginx TLS Gateway -> Jarvis Core

Jarvis Core will own all future connections to LiteLLM, n8n, Voice Service, Codex Bridge, MCP/agents and infrastructure adapters. Secrets belong in OpenBao and must never enter source control, Desktop configuration, prompts, model context, logs, or tool output.

Sensitive actions must follow:

    Structured Request -> Schema Validation -> Policy Engine -> Authorization
    -> Restricted Executor -> Credential Broker -> OpenBao -> Target
    -> Verification -> Audit

There is no direct LLM-to-shell path.

## Development

Requirements: Node.js/npm, Rust, and the native dependencies required by Tauri.

    cd apps/desktop
    npm ci
    npm test
    npm run build
    cargo test --manifest-path src-tauri/Cargo.toml
    npm run tauri dev

See [architecture](docs/architecture.md), [security policy](SECURITY.md), and [roadmap](docs/roadmap.md).
