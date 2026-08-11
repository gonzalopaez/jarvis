# JARVIS deployment status

Last verified against the live server: **2026-08-11**.

This file records what was tested directly against the running services, not what
the code is expected to do. The deployed `RestrictedExecutor` remains intentionally
disabled: JARVIS still only observes, explains and proposes.

## Live infrastructure map (Proxmox, node `Proxmox` / 192.168.1.5)

| Component | CT | Address | State |
|---|---|---|---|
| jarvis-core (+ jarvis-codex) | 124 | 192.168.1.21:4100 | active, READY |
| jarvis-voice | 125 | 192.168.1.22:4200 | active |
| jarvis-mcp | 126 | 192.168.1.23:4300 | active |
| prometheus | 127 | 192.168.1.24:9090 | active |
| wazuh relay | 120 | 192.168.1.10:5515 | active (token-protected) |
| openbao | 123 | 192.168.1.20:8200 | active, unsealed |
| n8n | 112 | 192.168.1.15:5678 | active |
| litellm | — | 192.168.1.11:4000 | active |
| nginx proxy manager (HUD ingress) | 103 | 192.168.1.4 | active |

## 1. HUD ingress via Nginx Proxy Manager — DONE

- HUD served at `https://jarvis.d4rkn0d3.com` (internal-only ACL, `npm-3` Let's Encrypt cert).
- AdGuard (192.168.1.2) resolves `jarvis.d4rkn0d3.com → 192.168.1.4`.
- The stale accumulated build was replaced with a clean rebuild; the site now
  serves `assets/index-kH0wNBBc.js` (HTTP 200), which includes the voice-alert
  announcement frontend.
- `/v1/health` → `{"status":"ready"}`; `/api/v1/health` → 200 with core and codex
  `READY` (voice/n8n/mcp still report `not_connected` — Core wiring, out of scope here).
- `/ws` returns 401 to unauthenticated clients (correct: the gateway is authenticated).
- CT124 firewall permits 4100 only from the NPM workload (192.168.1.4);
  `JARVIS_WEB_ORIGIN=https://jarvis.d4rkn0d3.com` is set and exact.

## 2. Real Prometheus telemetry for down-detection — DONE

- The Core queries `up == 0 or jarvis_proxmox_guest_up == 0 or jarvis_proxmox_service_up == 0`.
  The two `jarvis_proxmox_*` gauges previously had no source.
- Added them via the node-exporter textfile collector on the Proxmox host
  (see [ADR-011](docs/adr/ADR-011-proxmox-textfile-exporter.md)). The exporter
  script and units are now version-controlled under `deploy/`.
- `jarvis_proxmox_service_up` now covers the JARVIS services (core, codex, voice,
  mcp, prometheus, wazuh relay), not just the three original non-JARVIS services;
  `jarvis_proxmox_guest_up` now also covers CT127 (prometheus).
- Verified: Prometheus ingests 14 guest_up + 9 service_up series; the Core
  down-query returns exactly the two intentionally-stopped guests (`dc`, `freeipa`).
- Note: the Core does not expose `/metrics`; it is a Prometheus consumer, so there
  is no Core application-metrics scrape job (the original task assumption did not apply).

## 3. MCP gateway allow-list — VERIFIED CORRECT + REGRESSION TEST ADDED

- The Proxmox pool `JARVIS` contains only CT124 and CT125, so the existing
  `ALLOWED_VMIDS = {124, 125}` was already correct; 126/127 are the MCP and
  Prometheus themselves and are outside the pool.
- Removed the duplicated literal: the `proxmox.vm.status` schema `enum` now derives
  from `ALLOWED_VMIDS` (single source of truth).
- Added `services/mcp-gateway/test_jarvis_mcp.py`: a query for a VMID outside the
  allow-list is rejected before any Proxmox call, and the schema enum must track
  the allow-list. Wired into CI as the new `mcp` job.

## Still pending (unchanged, out of scope for this pass)

Persistence, definitive identity (OIDC/WebAuthn/MFA), OpenBao integration,
controlled write actions, n8n workflows, and reproducible release/rollback tooling
remain open. The RestrictedExecutor stays disabled.
