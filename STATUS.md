# JARVIS deployment status

Last verified against the live server: **2026-08-11**.

This file records what was tested directly against running services, not what
the code is expected to do. The deployed `RestrictedExecutor` remains
intentionally disabled: JARVIS observes, explains and proposes, but does not
execute infrastructure or containment changes.

## Live infrastructure map

Proxmox node: `Proxmox` (`192.168.1.5`).

| Component | CT | Address | Verified state |
|---|---:|---|---|
| jarvis-core (+ jarvis-codex) | 124 | `192.168.1.21:4100` | active, READY |
| jarvis-voice | 125 | `192.168.1.22:4200` | active |
| jarvis-mcp | 126 | `192.168.1.23:4300` | active |
| prometheus | 127 | `192.168.1.24:9090` | active |
| wazuh relay | 120 | `192.168.1.10:5515` | active, token-protected |
| openbao | 123 | `192.168.1.20:8200` | active, unsealed |
| n8n | 112 | `192.168.1.15:5678` | active |
| litellm | — | `192.168.1.11:4000` | active |
| nginx proxy manager | 103 | `192.168.1.4` | active, HUD ingress |

## HUD ingress

- The HUD is served at `https://jarvis.d4rkn0d3.com` behind the internal-only
  Nginx Proxy Manager ACL and the `npm-3` Let's Encrypt certificate.
- AdGuard resolves `jarvis.d4rkn0d3.com` to `192.168.1.4`.
- A clean frontend rebuild replaced the stale accumulated build. The served
  `assets/index-kH0wNBBc.js` returned HTTP 200 and includes voice-alert
  announcement support.
- `/v1/health` returned `{"status":"ready"}`. `/api/v1/health` returned HTTP
  200 with Core and Codex `READY`; voice, n8n and MCP still reported
  `not_connected` through Core at verification time.
- `/ws` rejected an unauthenticated client with HTTP 401.
- CT124 permits port 4100 only from the NPM workload, and
  `JARVIS_WEB_ORIGIN=https://jarvis.d4rkn0d3.com` is configured exactly.

## Prometheus down-detection

- Core queries
  `up == 0 or jarvis_proxmox_guest_up == 0 or jarvis_proxmox_service_up == 0`.
- The `jarvis_proxmox_*` gauges are produced through the node-exporter textfile
  collector on the Proxmox host (ADR-011). The exporter script and systemd
  units are version-controlled under `deploy/`.
- `jarvis_proxmox_service_up` covers Core, Codex, Voice, MCP, Prometheus and the
  Wazuh relay. `jarvis_proxmox_guest_up` includes CT127.
- Prometheus ingested 14 `guest_up` and 9 `service_up` series. The Core
  down-query returned exactly the two intentionally stopped guests, `dc` and
  `freeipa`.
- Core is a Prometheus consumer and does not expose an application `/metrics`
  endpoint.

## MCP gateway allow-list

- The Proxmox pool `JARVIS` contains CT124 and CT125. The existing
  `ALLOWED_VMIDS = {124, 125}` was verified as correct; CT126 and CT127 host MCP
  and Prometheus and are outside that pool.
- The `proxmox.vm.status` schema enum now derives from `ALLOWED_VMIDS` instead
  of duplicating the values.
- `services/mcp-gateway/test_jarvis_mcp.py` verifies that an out-of-scope VMID
  is rejected before any Proxmox call and that the schema tracks the allow-list.
  CI includes the corresponding `mcp` job.

## LiteLLM SOC control plane (ADR-012)

- `/v1/models` listed `jarvis-soc-l1` (Ollama `llama3.2`) and
  `jarvis-soc-l2` (Ollama `qwen2.5`) alongside the seven pre-existing aliases.
  The deployed configuration was backed up before the additive change.
- Both SOC aliases returned schema-valid structured JSON against
  `security-verdict.v1.schema.json` in manual tests.
- The scoped `n8n-soc-triage` key is restricted to the two SOC aliases, with a
  budget of 5 per 24 hours and an RPM limit of 60. An out-of-scope model call
  was rejected with HTTP 401.
- The separate `n8n-soc-embeddings` key is restricted to `nomic-embed-text`,
  with a budget of 2 per 24 hours and an RPM limit of 60.
- The active `SOC 2.0` workflow was backed up, patched in place, published and
  tested with synthetic L1 and L2 alerts. Executions `2333` and `2338`
  completed; L2 traversed Wazuh enrichment, LiteLLM, parsing, Telegram branches
  and Qdrant persistence with `needsReview=false` and `ACTION_ENABLED=false`.
- The stale embeddings token, missing Wazuh credential reference and incorrect
  Qdrant `POST /points` method were repaired. Credentials remain outside Git.
- After the LiteLLM restart, Core and Codex remained `READY` and both
  `jarvis-fast` and the repaired `jarvis-reasoning` route responded.

## Known gaps

- Forwarding SOC `proposed_actions` to Core is not implemented.
- `PolicyEngine` still lacks the real tiered capability rules defined by the
  proposed multi-agent ADR.
- Definitive identity (OIDC/WebAuthn/MFA), OpenBao integration, controlled write
  actions and reproducible release/rollback tooling remain open.
- `RestrictedExecutor` remains disabled.
