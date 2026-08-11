# ADR-010: Transitional hashed Bearer-token authentication

- Status: Accepted for development and initial private integration
- Date: 2026-08-08

## Decision

Support opaque high-entropy Bearer credentials through a server-side registry containing SHA-256 digests, subjects and fixed roles. Compare digests in constant time and never accept identity claims from request JSON.

## Consequences

The repository stores no credential values or production digest registry. TLS is mandatory because possession authenticates the caller. Credential rotation and bootstrap remain operational requirements.

This adapter is transitional. Privileged service-to-service links should move to mTLS or workload identity with OpenBao-backed lifecycle management.
