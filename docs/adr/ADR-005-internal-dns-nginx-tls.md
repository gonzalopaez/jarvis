# ADR-005: Internal DNS and Nginx TLS gateway

- Status: Accepted for future implementation
- Date: 2026-08-08

## Decision

Use AdGuard internal DNS and Nginx as the TLS reverse proxy with the existing wildcard certificate.

## Consequences

Configuration uses service names rather than workload IPs. Services remain private by default, production traffic is encrypted and privileged links use mTLS where appropriate.
