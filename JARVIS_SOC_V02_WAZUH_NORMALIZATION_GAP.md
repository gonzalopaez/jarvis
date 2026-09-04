# Wazuh Normalization Gap

Date: 2026-09-04
Scope: static code analysis plus read-only production service/config inventory; no production alert rows/logs read.

## Implementations

1. Relay: `services/wazuh-relay/jarvis_wazuh_relay.py`, function `normalized`.
2. Domain agent: `services/wazuh-agent/wazuh_agent.py`, function `normalize_alert`.
3. n8n: `integrations/n8n/SOC_2_0_correlation.json`, Code node `Normalize Alert Data`.
4. Core adds a fourth narrowing boundary in `services/core/src/security.rs::collect`, whose `Alert` struct accepts only id/host/timestamp/severity/title/description/source_ip.

## Production role

| NORMALIZER | LIVE STATUS | ROLE |
|---|---|---|
| Wazuh Agent `normalize_alert` | USED_IN_PRODUCTION / PRIMARY FOR CASES | Active service on CT120 port 5515; deployed source SHA-256 matches repo; Core polls its `/alerts` endpoint |
| n8n `Normalize Alert Data v3` | USED_IN_PRODUCTION / PARALLEL | Active `SOC 2.0` webhook path for 5m correlation + Telegram; not the source for `SocCaseStore` |
| standalone relay `normalized` | AVAILABLE_BUT_NOT_PRIMARY | Unit installed but inactive; deployed file differs from repo |
| Core poller narrowing | USED_IN_PRODUCTION / PRIMARY FOR CASES | Converts Wazuh Agent response into `security.alert`, then EventBus feeds `SocCaseStore` |

Verified transactional path: Wazuh alerts file → active `jarvis-wazuh-agent` `/alerts` → Core `WazuhSecurityPoller` → `security.alert` EventBus → `SocCaseStore`.

Legend: ✓ preserved; ~ transformed/defaulted; — dropped.

| FIELD | RELAY | AGENT | N8N | CORE POLLER | REQUIRED SOC v0.2 |
|---|---|---|---|---|---|
| alert_id | ✓ `id`, fallback full_log prefix | ✓ `id`, fallback full_log/UUID | ~ `id` fallback `rule.id` | ✓ as `id` | string/null; no random identity |
| timestamp | — original dropped; read time as `timestamp_ms` | ✓ string, but `timestamp_ms` becomes read time | ✓ original or current time fallback | only `timestamp_ms` | original RFC3339 + derived epoch or null |
| agent.id | — | ~ string, default `000` | — | — | string/null |
| agent.name | ✓ flattened `host`, default `unknown` | ✓ flattened `host`, default `unknown` | ✓ flattened `host`, default `unknown` | ✓ host | string/null |
| host | ✓ derived | ✓ derived | ✓ derived | ✓ | compatibility projection; null if absent |
| rule.id | — | ✓ flattened `rule_id` | ✓ flattened `rule_id` | — | string/null |
| rule.level | ✓ only used + severity | ✓ `level` | ✓ `level` | — level; ✓ severity | integer/null + derived severity |
| rule.description | ✓ title | ✓ title | ✓ title | ✓ title/description | string/null |
| rule.groups | — | — | live v3 flattens to comma text; repo drops | — | array, empty only when supplied empty; otherwise null |
| rule.frequency | — | — | — | — | integer/null |
| rule.mitre.id | — | — | live v3 flattens IDs; repo drops | — | array/string normalized to MITRE entries, no inference |
| rule.mitre.tactic | — | — | — | — | array/string/null aligned safely |
| rule.mitre.technique | — | — | — | — | array/string/null aligned safely |
| srcip | — | ✓ `source_ip` | — | ✓ field supported but relay omits it | string/null |
| dstip | — | — | — | — | string/null |
| srcuser | — | ~ may become generic `user` | ~ may become generic `user` | — | string/null |
| dstuser | — | ~ preferred into generic `user` | ~ preferred into generic `user` | — | string/null |
| process | — | — | — | — | string/null |
| parent_process | — | — | — | — | string/null |
| command_line | — | — | — | — | string/null, bounded |
| file | — | — | — | — | string/null |
| hash | — | — | — | — | structured known algorithms or value/null |
| decoder | — | — | — | — | object name/parent or null |
| location | — | — | — | — | string/null |
| raw_reference | — | — full_log copied as description | — full_log embedded | — | reference metadata only; not huge raw log |

## Precise loss points

- On the primary case path, MITRE is discarded first by Wazuh Agent and again by Core narrowing. The parallel live n8n v3 reads MITRE IDs but flattens them and never sends them to `SocCaseStore`.
- Original event time is discarded by relay and replaced by `time.time()` on each poll. Agent preserves a timestamp string but independently sets `timestamp_ms` to normalization time.
- Relay computes no source IP, so Core's source IP support is ineffective on its current production-shaped path.
- Agent/n8n collapse srcuser/dstuser into one ambiguous `user`, preventing relationship analysis.
- Agent invents UUID alert IDs and default agent ID `000`; all paths invent host `unknown`, contrary to v0.2 null semantics.
- n8n uses `rule.id` as a fallback alert ID, which can collapse distinct alerts.
- n8n correlation is ephemeral 5-minute host+user state. Core case correlation is persistent 30-minute host only. These serve different purposes but are currently easy to confuse.

## Current case-manager behavior

Static analysis of `SocCaseStore` shows:

- Only `security.alert` events with severity `critical` or `high` are ingested.
- Asset criticality lookup is case-insensitive by host; missing asset becomes `unknown`.
- Existing case key lookup: status in lowercase `open|investigating`, same host case-insensitively, `last_seen >= alert_time - 30 minutes`; latest wins. The 30-minute window is hardcoded.
- A duplicate alert ID already in the selected case returns its case ID and inserts no event.
- Same host/nonduplicate alert updates severity, priority, title, last_seen, source IP array, alert ID array and updated_at; then inserts one `case_events` evidence row.
- New-case `case_key` is lowercased host plus the absolute 30-minute epoch bucket. Boundary behavior may create a new key even when rolling-window lookup semantics overlap; live unique constraints are unknown.
- Priority is Critical→P1, High+Critical asset→P1, other High→P2.
- Evidence is the complete narrowed Core event payload, not the original Wazuh document.
- Live CT133 confirms legacy `confidence`, `mitre_techniques` and `assigned_to` columns, but this module does not populate confidence/MITRE/analyst lifecycle. Evidence contains only the narrowed Core payload.

## Recommendation

Adopt one canonical event at the Core boundary, version it, and make relay/agent/n8n produce or consume that contract incrementally. Keep compatibility projections (`id`, `host`, `severity`, `title`, `description`, `source_ip`, `timestamp_ms`) until `SocCaseStore` is migrated. Do not use MITRE inference when Wazuh provides none.
