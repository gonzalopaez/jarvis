# ADR-004: OpenBao for secrets

- Status: Accepted for future implementation
- Date: 2026-08-08

## Decision

Use OpenBao as the central secrets and future PKI/dynamic-credential platform.

## Consequences

Each workload has a least-privilege identity. Models request capabilities and never receive secrets. Git contains only non-sensitive examples and sanitized configuration.
