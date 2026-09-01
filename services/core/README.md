# Jarvis Core

Jarvis Core is the security and policy boundary behind the single Desktop API.

## Implemented foundation

- strict versioned request types;
- bounded schema and correlation-ID validation;
- rejection of secret-shaped structured action fields;
- explicit authenticated principal context;
- exact capability/target rules and role checks;
- deny-by-default policy;
- mandatory authorization boundary for protected actions;
- restricted executor trait with verified-result requirement;
- sanitized audit events;
- routed conversation responses through configured private services, with a fail-closed mock fallback when no conversation service is installed.
- exact HTTP routes for health and Core requests;
- opaque transport authentication interface;
- body-size, content-type, method and request-deadline controls;
- no-store/nosniff response headers and sanitized transport errors;
- optional Hyper listener through the network-server feature.
- hashed Bearer-token authentication with server-owned subjects and roles;
- enforced loopback/private/unique-local bind policy.
- canonical `/api/v1/health`, `/api/v1/agents` and `/api/v1/requests` routes;
- normalized health for the eight server components without simulated readiness;
- temporary compatibility for the original `/v1` health and request routes.
- bounded in-process Event Bus with normalized versioned envelopes;
- authenticated `/ws` upgrade with exact Origin validation, snapshot, heartbeat and lag resynchronization.
- bounded, expirable and revocable opaque browser sessions stored as digests;
- exact-Origin and CSRF enforcement for cookie-authenticated writes.
- normalized Telemetry Service with bounded collection intervals and adapter deadlines as a required boundary;
- explicit unavailable Prometheus adapter that never emits fabricated metrics.
- Tier 1 read, Tier 2 reversible-containment and Tier 3 infrastructure rules;
- single-use, session-scoped grants and exact Tier 3 resource confirmation;
- proposal-only Wazuh and Proxmox agent boundaries;
- parallel, deadline-bounded cross-domain evidence collection.

The executable fails closed unless `JARVIS_CORE_BIND` contains a validated private socket address and systemd provides `auth-registry.json` through `CREDENTIALS_DIRECTORY`. The registry contains only SHA-256 digests with server-owned subjects and roles; raw Bearer values are prohibited. The deployment unit binds explicitly to the private Core workload address. TLS termination and trusted proxy handling remain separate reviewed work.

The included Bearer adapter accepts only opaque credentials of at least 32 bytes and retains SHA-256 digests rather than raw values. It is a transitional private-integration mechanism; see docs/authentication.md.

The Codex expert-agent adapter is connected and reports `READY`; the Prometheus
telemetry adapter consumes real `jarvis_proxmox_guest_up` / `jarvis_proxmox_service_up`
metrics (see ADR-011). No n8n, OpenBao, shell or infrastructure *write* adapter is connected.

The deployed executor is intentionally disabled. Agents can forward structured
proposals to Core, where policy and authorization are enforced, but no
infrastructure or containment action can execute until a capability-specific
executor passes a separate review. See `tier_1_is_allowed_immediately`,
`domain_agent_cannot_issue_its_own_grant` and
`domain_agent_cannot_submit_human_confirmation`; implementation baseline
`a2f37e0`.

## LiteLLM request budget

Core has one direct LiteLLM adapter, `VoicePipeline`, which is used by the
conversation routes in `conversation.rs` and by the voice pipeline. Every chat
completion goes through `request_completion`, whose dedicated HTTP client has a
20-second total deadline. The serialized `messages` array is rejected above
12 KiB before the request is sent, and upstream responses are capped at 128 KiB.
Tool calls use the same deadline and response cap but do not carry model context.

`CodexHttpClient` in `conversation.rs` talks only to the separately bounded
Codex service; it is not a direct LiteLLM client. Its task timeout and 128 KiB
response cap are enforced independently.

## Deployment

The Desktop HUD is served read-only through Nginx Proxy Manager at
`https://jarvis.d4rkn0d3.com`; the Core answers `/v1/health`, `/api/v1/health`
and the authenticated `/ws` upgrade behind it. See `STATUS.md` at the repo root
for the live, verified deployment state.

WebSocket startup additionally requires `JARVIS_WEB_ORIGIN` as one exact HTTPS origin without a trailing slash. The gateway fails closed when missing, rejects anonymous or cross-origin upgrades and does not provide a browser authentication bypass.

## Test

    cargo test -p jarvis-core
    cargo test -p jarvis-core --features network-server
    cargo clippy -p jarvis-core --all-features --all-targets -- -D warnings
