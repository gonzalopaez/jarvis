# ADR-002: LiteLLM as AI Control Plane

- Status: Accepted for future implementation
- Date: 2026-08-08

## Decision

Place model routing and future MCP/Agent Gateway controls behind LiteLLM. Jarvis Core, not Desktop, is its client.

## Consequences

Provider selection, keys, routing and cost/policy controls remain backend concerns. No integration is active in v0.1-clean.
