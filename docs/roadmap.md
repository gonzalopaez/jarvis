# Roadmap

Baseline: `origin/main@a2f37e0`.

## Completed in the baseline

1. Secure repository foundation, Desktop HUD, Event Bus and tests.
2. Single versioned Core API and private transport boundaries.
3. LiteLLM aliases, governed model routing and bounded Codex adapter.
4. Private STT/TTS Voice service.
5. Sanitized n8n correlation workflow and Wazuh Agent proposal forwarding.
6. MCP read gateway and server-side Prometheus telemetry.
7. ADR-014 stages 0–5: capability catalog, Tier 1/2/3 policy, Wazuh Agent,
   Proxmox Agent, Core HTTP authorization and parallel evidence fan-out.

Evidence for item 7: PR #4 / `e54124c`, merge `199037a`, PR #5 / `627ad43`,
merge `4131336` and merge `a2f37e0`. Production deployment status is tracked
separately in `STATUS.md`.

## Pending

1. Capability-specific restricted write executors and reviewed rollout.
2. OIDC/WebAuthn/MFA and OpenBao credential-broker integration.
3. HUD representation of the Wazuh and Proxmox agents and typed Tier 3
   confirmation.
4. Reproducible release, approval and rollback tooling.
5. Audit and reconciliation of the unmerged voice-latency/GPU and Qdrant/RAG
   branches; neither is part of this baseline.
