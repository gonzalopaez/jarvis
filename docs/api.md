# Jarvis Core API

Jarvis Core provides the single versioned HTTPS/WSS boundary for the Web UI and transitional Tauri client.

## v1 envelope

Requests carry api_version, request_id, session_id and exactly one supported body kind:

- conversation: bounded text accepted by a mock-only path in this phase;
- action: a capability, target and non-sensitive structured parameters.

Authentication is transport metadata, not a body field. Unknown JSON fields, invalid identifiers, oversized/deep parameters and secret-shaped action fields are rejected before policy evaluation.

Responses contain correlation IDs, a sanitized status, an audit ID and either safe data or a stable error envelope. They never expose stack traces, headers, credentials, downstream topology or raw executor failures.

## Action lifecycle

    authenticated principal -> schema validation -> exact policy rule
    -> role check -> authorization boundary when required
    -> restricted executor -> verification -> sanitized audit

Missing rules deny by default. An authorization-required response never invokes an executor. There is no generic shell capability.

Schemas are stored under contracts/api; sanitized audit events are under contracts/events.

## HTTP transport foundation

The canonical Phase 2 routes are:

- GET `/api/v1/health`: aggregate operational health and canonical JARVIS state;
- GET `/api/v1/agents`: authenticated normalized agent inventory;
- GET `/api/v1/session`: authenticated browser session status and CSRF value;
- POST `/api/v1/requests`: authenticated Core request processing.

Compatibility routes retained temporarily are:

- GET /v1/health: minimal readiness and API version, without topology details;
- POST /v1/requests: authenticated Core request processing.

Other paths return 404 and other methods return 405 with an Allow header. Core requests require application/json, an opaque authorization credential interpreted by a server-side Authenticator, and a body within the configured limit. Responses use no-store and nosniff headers.

The transport has a request deadline that currently protects asynchronous body receipt and routing. Executors are synchronous and mock-only in this phase; a future network executor must use an async or isolated bounded execution model so blocking work cannot bypass deadlines.

The Hyper listener is created only through the private bind validator and has no default address. Production traffic terminates TLS at the internal Nginx gateway; direct public binding is prohibited. Browser routes are same-origin, so permissive CORS is neither required nor enabled.

The listener API now requires a validated private listener. Loopback, RFC1918 IPv4 and IPv6 unique-local addresses are accepted; unspecified and public addresses fail closed.

The initial real Authenticator maps hashed opaque Bearer credentials to server-owned subjects and roles. Request payloads cannot provide identity. See authentication.md for lifecycle requirements and migration direction.

The aggregate health endpoint reports unavailable components honestly. It does not turn placeholders into healthy mock agents. The authenticated agent endpoint returns the same normalized inventory for future realtime snapshots.

There is deliberately no public session-creation route. Session issuance belongs to a future trusted identity adapter. Cookie-authenticated POST requests require the exact Origin and `X-Jarvis-CSRF`; Bearer-authenticated service requests retain their existing transport boundary.
