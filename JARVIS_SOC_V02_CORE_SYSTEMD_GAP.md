# CT124 JARVIS Core systemd Gap

Status: **VERIFIED READ-ONLY**
Observed: 2026-09-04 through Proxmox `pct exec 124`

## Deployed state

- CT124 `jarvis-core`: running.
- `jarvis-core.service`: active/running since 2026-09-02 16:37:54 UTC.
- Runtime `jarvis-core:jarvis-core`; binary `/usr/local/bin/jarvis-core`.
- Base unit `/etc/systemd/system/jarvis-core.service`.
- Drop-ins: `codex.conf`, `prometheus.conf`, `soc-db.conf`, `wazuh.conf`.
- Base hardening includes NoNewPrivileges, empty capability sets, address-family/system-call restrictions and UMask 0077.

## SOC database credential

- `soc-db.conf` declares the private CT133 database URL.
- Logical credential `soc-db-password` is sourced from `/etc/jarvis-core/soc-db-password`.
- Safe metadata: root:root, mode 0400, regular file.
- Credential contents were never read.

## Repository vs deployed

| ITEM | REPOSITORY | DEPLOYED | RESULT |
|---|---|---|---|
| Base unit | No SOC DB credential | Same pattern | MATCH |
| SOC DB drop-in | absent | `soc-db.conf` | CONFIGURATION DRIFT |
| Wazuh drop-in | example-level config | `wazuh.conf`, endpoint `/alerts` | LIVE DROP-IN NOT VERSIONED |
| Core state | not provable from repo | active/running | VERIFIED |

The startup concern is resolved operationally: `soc-db-password` is correctly wired. Reproducibility remains a gap because the sanitized drop-in is absent from the repo. No unit or service was changed.
