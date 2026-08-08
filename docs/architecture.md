# Architecture

## System boundary

    Desktop
      -> HTTPS/WSS
      -> AdGuard internal DNS
      -> Nginx TLS Gateway
      -> Jarvis Core
           -> LiteLLM: Model, MCP and Agent Gateway
           -> n8n: automation, integrations and long-running workflows
           -> Voice Service: STT and TTS
           -> Codex Bridge
           -> future AI Hub / agents
           -> Policy Engine / restricted executors
           -> credential broker -> OpenBao

The Desktop depends only on the stable Jarvis Core API. It never receives internal service addresses, provider credentials, infrastructure credentials or OpenBao access.

## Current baseline

Only apps/desktop is implemented. External components are boundaries and mocks, not active integrations. Future services communicate through versioned contracts in contracts/.

Infrastructure workloads will run in isolated Proxmox VMs/LXCs with explicit firewall rules. No service is public unless a documented requirement, threat review and authorization justify exposure.
