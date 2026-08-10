# Changelog

All notable changes follow Keep a Changelog principles and semantic versioning.

## [Unreleased]

### Added

- Server Telemetry Service with strict operational metrics, bounded intervals and adapter interfaces.
- Normalized `telemetry.snapshot` and `telemetry.source.status` realtime events.
- Browser realtime telemetry mapping without Prometheus/Wazuh access or polling.
- Bounded opaque Session Store with digest-only storage, TTL, revocation and hardened cookies.
- Browser realtime client with session probe, strict envelope validation, visibility handling and jittered reconnect backoff.
- CSRF and exact-Origin enforcement for cookie-authenticated writes.
- Bounded server Event Bus and normalized realtime event envelope contract.
- Authenticated `/ws` gateway with exact Origin validation, initial snapshot, heartbeat and slow-consumer resynchronization.
- Canonical `/api/v1` health, agent-inventory and request routes with temporary `/v1` compatibility.
- Aggregate health and agent inventory contracts for the eight server components.
- Explicit browser-session security boundary that fails closed until trusted identity issuance exists.
- Runtime-neutral Web UI client with separate browser and transitional Tauri adapters.
- Same-origin static Web UI ingress and browser health checks without frontend credentials.
- Canonical operational state presentation tokens and browser-runtime security tests.
- Realtime architecture boundary documentation for the future authenticated WebSocket gateway.
- Versioned Core request, response and sanitized audit-event schemas.
- Deny-by-default Core policy engine, authentication boundary and restricted executor contract.
- Security tests for validation, authorization, execution verification and audit behavior.
- Minimal Hyper transport with exact routes, opaque authentication, bounded bodies, deadlines and sanitized errors.
- Hashed Bearer-token authentication with constant-time comparison and server-owned identities.
- Listener policy that rejects unspecified and public bind addresses.
- Fail-closed Core executable with graceful shutdown, systemd credentials and a hardened service unit.
- Internal Nginx TLS ingress configuration for the private Core listener.
- Desktop-owned HTTPS Core client with runtime credential loading, health polling and real conversation routing to the mock-only Core path.
- Read-only CI for Core, Desktop frontend, JSON contracts and secret scanning.

### Changed

- Agent Matrix now uses the normalized service-state vocabulary and reports unavailable integrations as real offline boundaries rather than simulated staged values.
- Idle, offline and background UI rendering avoids continuous canvas/CSS animation; active waveform rendering is rate-limited.
- The Web UI is responsive below the former fixed 1180px desktop minimum.

## [0.1.0-clean] - 2026-08-08

### Added

- Clean repository foundation and modular directory layout.
- Approved Tauri cinematic HUD with real local Linux telemetry.
- Typed Event Bus, provider-neutral mocks and baseline tests.
- Security, network, secrets, API, voice, Proxmox, threat-model and roadmap documentation.
