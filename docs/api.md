# Jarvis Core API

Jarvis Core provides the future Desktop single versioned HTTPS/WSS boundary. The transport listener is intentionally deferred until the domain contract and policy path are stable.

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

The implemented routes are:

- GET /v1/health: minimal readiness and API version, without topology details;
- POST /v1/requests: authenticated Core request processing.

Other paths return 404 and other methods return 405 with an Allow header. Core requests require application/json, an opaque authorization credential interpreted by a server-side Authenticator, and a body within the configured limit. Responses use no-store and nosniff headers.

The transport has a request deadline that currently protects asynchronous body receipt and routing. Executors are synchronous and mock-only in this phase; a future network executor must use an async or isolated bounded execution model so blocking work cannot bypass deadlines.

The optional Hyper listener accepts a caller-provided bound socket. There is deliberately no executable, default address, CORS policy, trusted-proxy policy or TLS configuration yet. Production traffic will terminate TLS at the internal Nginx gateway; direct public binding is prohibited.
