# ADR-006: Single Jarvis Core API

- Status: Accepted
- Date: 2026-08-08

## Decision

Desktop knows only the versioned Jarvis Core HTTPS/WSS endpoint.

## Consequences

Backend topology can evolve without Desktop changes. LiteLLM, n8n, voice, OpenBao, Wazuh, Proxmox and MCP addresses and credentials never enter Desktop configuration.
