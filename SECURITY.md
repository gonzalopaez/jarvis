# Security policy

## Principles

JARVIS is designed around least privilege, deny by default, explicit authorization, defense in depth, secure defaults and auditable decisions. Production traffic is encrypted. Privileged service-to-service traffic should use mTLS where appropriate.

Hardcoded credentials are prohibited. OpenBao is the central secrets system. Secrets must never become model context or appear in prompts, logs, traces, errors, workflow exports, committed configuration, Desktop state, or tool results.

## Action safety

Model output is untrusted data. No LLM or agent may execute a shell command directly. Sensitive actions require a structured request, schema validation, policy evaluation, authorization, a narrowly restricted executor, brokered credentials, target verification and an audit record. Inputs are validated at every trust boundary and logs are structured, minimal and sanitized.

## Infrastructure

Services remain private by default. Internal DNS is provided by AdGuard and TLS ingress by Nginx using managed certificates. Each VM/LXC has an explicit firewall policy. Administrative interfaces, OpenBao, Proxmox, Wazuh and MCP servers are never exposed to Desktop.

## Reporting

Do not open a public issue containing a vulnerability or sensitive data. Report privately to the repository owner using GitHub private vulnerability reporting when enabled.
