# ADR-009: Core domain before transport

- Status: Accepted
- Date: 2026-08-08

## Decision

Implement and test Core validation, policy, authorization boundaries, execution contracts and audit behavior before selecting or exposing an HTTP/WebSocket server.

## Consequences

Transport authentication can later map into an explicit trusted principal context without leaking headers into domain requests. The domain remains framework-independent and testable. No network listener is included in the first feature/core-gateway increment.
