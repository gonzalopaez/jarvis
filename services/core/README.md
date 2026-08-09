# Jarvis Core

Jarvis Core is the security and policy boundary behind the future single Desktop API.

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
- mock-only conversation response.
- exact HTTP routes for health and Core requests;
- opaque transport authentication interface;
- body-size, content-type, method and request-deadline controls;
- no-store/nosniff response headers and sanitized transport errors;
- optional Hyper listener through the network-server feature.
- hashed Bearer-token authentication with server-owned subjects and roles;
- enforced loopback/private/unique-local bind policy.

No executable or default bind address is provided. A caller must request a validated private listener and supply a real authenticator. This prevents accidental unauthenticated startup or public exposure. TLS termination, trusted proxy handling and production identity remain separate reviewed work.

The included Bearer adapter accepts only opaque credentials of at least 32 bytes and retains SHA-256 digests rather than raw values. It is a transitional private-integration mechanism; see docs/authentication.md.

No provider, model, n8n, OpenBao, shell or infrastructure adapter is connected.

## Test

    cargo test -p jarvis-core
    cargo test -p jarvis-core --features network-server
    cargo clippy -p jarvis-core --all-features --all-targets -- -D warnings
