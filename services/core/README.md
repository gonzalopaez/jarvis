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

No HTTP listener, provider, model, n8n, OpenBao, shell, infrastructure or network adapter is connected.

## Test

    cargo test -p jarvis-core
    cargo clippy -p jarvis-core -- -D warnings
