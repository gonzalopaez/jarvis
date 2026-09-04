# CT133 PostgreSQL SOC — Verified Schema Analysis

Status: **VERIFIED READ-ONLY**
Observed: 2026-09-04 through Proxmox `pct exec 133`
Schema dump: `docs/discovery/JARVIS_SOC_CT133_SCHEMA_2026-09.sql`

## Server and database

- CT: 133 `jarvis-soc-db`, running.
- PostgreSQL: 15.19 (Debian 15.19-0+deb12u1).
- Databases: `jarvis_soc` and maintenance database `postgres`; metadata session user: `postgres`.
- Search path: `"$user", public`.
- Application objects: schema `public` only.
- Owners: all three tables and both sequences are owned by `jarvis_soc`.
- No views, materialized views, triggers, public functions or enum types.
- Migration history: **NONE**. No known migration-history table exists.
- No SOC rows were queried. Only catalog metadata and relation sizes were read.

## Objects

| DATABASE OBJECT | CODE EXPECTATION | LIVE STATE | MATCH | GAP | RECOMMENDED ACTION |
|---|---|---|---|---|---|
| `assets` | host + criticality lookup | PK `host text`; criticality check; owner/function/tags/updated_at | YES | Defaults not consumed by case manager | Preserve |
| `soc_cases.id` | generated bigint | bigint + owned sequence, PK | YES | None | Preserve |
| Case identity | `case_key` | text UNIQUE NOT NULL | YES | Bucket key and rolling lookup differ at boundaries | Test before changing logic |
| Lifecycle | lowercase open/investigating | check allows open/investigating/contained/closed | PARTIAL | Target has more states | Defer check changes to state-machine phase |
| Severity/priority | current enums | text checks exist | YES | `priority` combines initial/final | Add nullable projections; keep legacy |
| Host/time | host/first_seen/last_seen | exact columns; host/time index | YES | Case-insensitive query cannot fully exploit plain index | Preserve now |
| Sources/alerts | arrays | `source_ips text[]`, `alert_ids text[]` | YES | No DB uniqueness for alert ID | Preserve app dedup; normalize future evidence identity |
| MITRE | technique storage | `mitre_techniques text[]` | PARTIAL | Ingestion never populates; no tactic/relation/time | Keep projection; add structured assessment data |
| Confidence | confidence storage | legacy high/medium/low | PARTIAL | Not numeric/versioned | Preserve; use numeric assessment confidence |
| Analyst | assignment | `assigned_to text` | PARTIAL | No verdict/history/timestamps | Reuse and extend additively |
| `case_events` | event evidence | complete expected table and FK/index | YES | Narrowed payload; no schema/source identity | Add canonical payload/version safely |
| Views/triggers/functions/enums | none assumed | none | YES | None | Preserve |
| Migration history | none in repo | none live | YES | No order/checksum control | Add `jarvis_schema_migrations` explicitly |

Approximate relation sizes: assets 32 KiB, case_events 48 KiB, soc_cases 80 KiB. Low lock timeouts remain mandatory.

## Exact live columns

- `assets`: host, criticality, owner_name, asset_function, tags, updated_at.
- `soc_cases`: id, case_key, status, severity, priority, title, host, first_seen, last_seen, source_ips, alert_ids, mitre_techniques, confidence, assigned_to, created_at, updated_at.
- `case_events`: id, case_id, occurred_at, event_type, severity, title, evidence, created_at.

## Constraints and indexes

- PKs: `assets(host)`, `soc_cases(id)`, `case_events(id)`.
- Unique: `soc_cases(case_key)`.
- FK: `case_events.case_id → soc_cases.id ON DELETE CASCADE`.
- Checks: asset criticality; case confidence, priority, severity and status.
- Indexes: `case_events(case_id, occurred_at DESC)`, `soc_cases(host,last_seen DESC)`, `soc_cases(status,priority,last_seen DESC)` plus PK/unique indexes.
- Sequences: `soc_cases_id_seq`, `case_events_id_seq`; no identity columns.

## Readiness

There is no destructive conflict with an additive v0.2 model. Existing MITRE, confidence and priority columns must remain legacy/current projections rather than be repurposed incompatibly. Versioned values belong in append-only assessments.

**Phase 1 migration readiness: UNBLOCKED FOR LOCAL PREPARATION.** Production application remains prohibited pending SQL/locks/backup/compatibility/rollback review.
