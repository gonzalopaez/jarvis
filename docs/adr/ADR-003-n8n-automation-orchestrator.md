# ADR-003: n8n as automation orchestrator

- Status: Accepted for future implementation
- Date: 2026-08-08

## Decision

Use n8n for automations, integrations and long-running workflows. Normal conversation does not require n8n.

## Consequences

Jarvis Core chooses when to invoke n8n. Exported workflows must be sanitized and contain no credentials, credential IDs or environment-specific addresses.
