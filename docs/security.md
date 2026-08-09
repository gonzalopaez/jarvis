# Security architecture

The governing flow is:

    LLM / Agent -> Structured Request -> Schema Validation -> Policy Engine
    -> Authorization -> Restricted Executor -> Credential Broker -> OpenBao
    -> Target -> Verification -> Audit

Models propose; policy and authorization decide; restricted code executes. Executors expose allow-listed capabilities rather than arbitrary commands. Credentials remain behind the broker boundary and are never returned to models.

Controls include least-privilege identities, deny-by-default policies, input/output schemas, timeouts, bounded payloads, sanitized logs, correlation IDs, explicit approval for high-impact actions and postcondition verification.

The current Core foundation enforces exact capability/target rules. It refuses credential-shaped structured fields, performs no execution for unknown or authorization-pending actions, and requires executors to return a verified result. Authentication evidence and authorization grants will be bound to the future transport in a separate reviewed change; callers cannot self-assert approval in the request body.

The HTTP layer authenticates before buffering the request body, enforces exact routes/methods/content type, limits body size, applies a request deadline and never reflects credentials or invalid payloads in errors. The Authenticator returns server-defined principals and roles; request JSON cannot assert either. Health output is intentionally minimal.

No production listener is configured. Enabling a runnable Core requires a reviewed authenticator, loopback/private binding, Nginx TLS ingress, trusted-proxy rules and explicit network policy. Mock authenticators exist only in tests.
