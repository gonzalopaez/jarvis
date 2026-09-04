# CT112 n8n Live Inventory

Status: **VERIFIED READ-ONLY**
Observed: 2026-09-04 through Proxmox `pct exec 112`

## Runtime

- CT112 `n8n`, running at its current private address.
- Native `/usr/bin/n8n` systemd service, not Docker/Podman.
- Service active/running with `/etc/n8n/jarvis.env` drop-in reference; environment contents were not read.
- CLI exported workflow definitions only to `/tmp`; credentials were not exported and workflows were not executed.

## Inventory summary

- 33 workflows total.
- Active workflows relevant here: `SOC 2.0` and `JARVIS Core Gateway v0.1`.
- Multiple historical SOC L1/L2 workflows are inactive.
- No tags on the identified SOC workflows.

## SOC workflows

| ID | NAME | ACTIVE | UPDATED | CLASSIFICATION | RELEVANT NODES |
|---|---|---:|---|---|---|
| `E9ccQMeDylXIZ9dO` | SOC 2.0 | true | 2026-08-11 23:31 UTC | VERSIONED_BUT_DIFFERENT | Wazuh webhook, normalize v3, correlate 5m, Telegram |
| `JarvisCoreGateway01` | JARVIS Core Gateway v0.1 | true | 2026-08-08 07:21 UTC | LIVE_ONLY | Jarvis webhook, validation, LiteLLM, response |
| `xATX40KOCxFoDTMB` | Wazuh_SOC_L2_Automated | false | 2026-03-16 | INACTIVE | Wazuh/Wazuh API/Ollama/Telegram |
| several IDs | SOC_Professional_L1_L2_Orchestrator_v1 | false | 2026-03-16..21 | INACTIVE | Wazuh, Qdrant/embeddings, Ollama, Telegram |
| `j2SSDFyxu3SHzdXx`, `BMz0yg1PepC92fhC` | historical `My workflow 3/4` | false | 2026-03-21 | INACTIVE | includes disabled-by-workflow Tier2-like action branches |

The historical workflows containing `ACCIÓN: Disable User`, `Block IP` and `Isolate Host` are inactive. Their nodes were not invoked.

## Repo/live comparison for SOC 2.0

Structure and connection graph match the four-node repository export, but two node parameter hashes differ:

- `Normalize Alert Data`: live v3 differs from repo.
- `Telegram Correlated Alert`: live differs from repo.
- Webhook parameters, correlation node and connection graph match.
- Repo says `active:false`; live is `active:true`.

Live normalize v3 preserves rule ID/description/level, flattened groups and flattened MITRE IDs, FIM context, extracted user/source IP, timestamp/location/full log. It still fabricates default agent ID/name/IP and a current timestamp when absent. It uses a different uppercase severity scale. No secrets were present in or persisted from this analysis.

## Architectural conclusion

`SOC 2.0` is an active parallel notification/correlation workflow. The transactional case path does not pass through it: Core polls the active Wazuh Agent directly. n8n must not be modified as part of canonical event foundation until a separate reconciliation is approved.
