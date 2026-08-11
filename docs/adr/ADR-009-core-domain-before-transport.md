# ADR-009: Core domain before transport

- Status: Accepted
- Date: 2026-08-08

## Decision

Implement and test Core validation, policy, authorization boundaries, execution contracts and audit behavior before selecting or exposing an HTTP/WebSocket server.

## Consequences

Transport authentication maps an opaque authorization value into an explicit trusted principal context without leaking headers into domain requests. The domain remains framework-independent and testable.

The first transport increment provides an optional Hyper listener but no executable or bind address. Production identity, TLS ingress and trusted-proxy behavior require separate review before the service can run.
