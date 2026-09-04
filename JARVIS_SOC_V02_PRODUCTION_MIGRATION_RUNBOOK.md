# JARVIS SOC v0.2 — Production Migration Runbook

Status: READY_FOR_REVIEW (not approved for execution)

## Scope and evidence

This runbook covers only additive PostgreSQL migrations on CT133 (`jarvis-soc-db`, database `jarvis_soc`). It does not deploy Core v0.2, change Wazuh/n8n/LiteLLM, or enable actions. Fase 1.5 and 1.6 passed in NONPROD; production has not been modified.

Reviewed source commit: `7e02f42d2c3ff2e89b357b38bdc085c7f33f1232`.

Migration artifacts must match immediately before the window:

| File | SHA-256 |
|---|---|
| `services/core/migrations/0001_migration_history.sql` | `9509aab1a0a5913a983dd119a4ac14f1871d883576b75455b5b9b5a574a5fb3b` |
| `services/core/migrations/0002_soc_v02_foundation.sql` | `6de4fccf0f0f8d80e0a59197b46b65b4cd2ad45864fc0f0cc544f901e28ef294` |
| `scripts/soc-migrate.sh` | `89ad7394bad588fbe55b40a21cc0b96a2c85a6731bfcd301250e6d4bff5c70ad` |

## Live discovery gate

The attempted read-only SSH discovery is currently `UNVERIFIED` because the managed execution environment rejected the Proxmox socket (`Operation not permitted`). Therefore no live value is inferred here. An operator must capture the following before scheduling DDL:

```bash
pct status 133
pct config 133
pct exec 133 -- su - postgres -c "psql -d jarvis_soc -X -v ON_ERROR_STOP=1 -c 'SELECT version(), current_database(), current_user; SHOW server_version; SHOW data_directory; SHOW max_connections;'"
pct exec 133 -- su - postgres -c "psql -d jarvis_soc -X -Atc \"SELECT pg_size_pretty(pg_database_size('jarvis_soc'));\""
pct exec 133 -- su - postgres -c "psql -d jarvis_soc -X -c \"SELECT state,count(*) FROM pg_stat_activity WHERE datname='jarvis_soc' GROUP BY state;\""
```

Operator-provided read-only evidence verifies LIVE CT133 and the versioned baseline: both use the canonical `COLUMN`, `CONSTRAINT`, `INDEX` serialization, exactly 47 lines, and SHA-256 `ba004dd05ecc0bdc8023ef4e7830a65026c4229bb013ae6d3bdd045b97397f5c`. The baseline was restored on PostgreSQL 15.19/Debian 12 in a temporary Proxmox CT. `SCHEMA BASELINE = VERIFIED`; `SCHEMA DRIFT = NONE`; `LIVE SCHEMA FINGERPRINT = VERIFIED`. Evidence was collected by an external operator because Codex SSH is restricted. No CT133 DDL/DML was performed.

## Preconditions and abort criteria

Operator records UTC start time and ticket/request ID. Preconditions: CT ID 133, hostname `jarvis-soc-db`, database exactly `jarvis_soc`, PostgreSQL major 15, clean blocking-lock/idle-in-transaction review, sufficient disk, verified backup and tested restore path, no existing `jarvis_schema_migrations`, clean Core health, and exact artifact hashes above. Abort on any mismatch, unavailable backup/restore, unexpected migration history, lock contention, dangerous idle transaction, insufficient space, runner safety failure, checksum mismatch, migration failure, schema/postcheck mismatch, or Core health degradation. Do not kill sessions or increase `lock_timeout`.

## Backup and restore plan (operator executes in the approved window)

Use a consistent Proxmox backup/snapshot of CT133 according to the site's backup policy, plus a schema/data dump stored on Proxmox backup storage (never Omarchy). Supply credentials through the existing protected mechanism; never put passwords in argv or logs. Verify the dump is readable and restore it to an isolated PostgreSQL 15 target before the window. If backup or restore verification fails, abort.

## Execution (commands are documented, not executed)

From the reviewed checkout, set `JARVIS_ALLOW_SOC_MIGRATIONS=YES`, `JARVIS_SOC_MIGRATION_DATABASE_URL` through a protected environment/credential mechanism, and `JARVIS_SOC_MIGRATION_EXPECTED_DATABASE=jarvis_soc`. Verify hashes and target identity again, then run exactly:

```bash
JARVIS_ALLOW_SOC_MIGRATIONS=YES \
JARVIS_SOC_MIGRATION_EXPECTED_DATABASE=jarvis_soc \
JARVIS_SOC_MIGRATION_DATABASE_URL="$PROTECTED_LIBPQ_URL" \
scripts/soc-migrate.sh
```

The runner uses a transaction, advisory lock, `lock_timeout=2s` and `statement_timeout=30s`. Do not substitute hand-written SQL. A failure aborts the window; do not retry automatically.

## Postchecks and observation

Read-only checks must confirm one row each for 0001/0002 with the expected checksums, the new columns/tables/constraints/indexes, and preservation of all legacy columns. Confirm no unexpected row-count changes. Verify the existing `jarvis-core.service` remains active, health endpoint and DB connectivity are healthy, ingestion and dedup continue, and PostgreSQL errors/locks/connections do not spike. Observe for a human-approved interval (recommended 30 minutes minimum, or the site's established SOC window) before considering a later Core v0.2 deployment. DB migration is not Core deployment.

## Rollback and evidence

Because application writes may occur after DDL, rollback is operational: stop the change, keep additive objects, restore CT133/database from the verified backup only under an explicit human decision, and validate Core health. Do not invent a down migration. Preserve sanitized evidence: commit/hash table, pre/post fingerprints, migration history, durations, lock observations, backup/restore IDs, health checks and operator/ticket. Never persist secrets or production case data.

Final gate is `PRODUCTION MIGRATION RUNBOOK = READY_FOR_REVIEW` and `PRODUCTION DDL = NOT APPROVED FOR EXECUTION`. A separate human approval is required for execution; this document authorizes no production command.

## Runner authorization review

`scripts/soc-migrate.sh` has a deliberate fail-closed gate: `JARVIS_ALLOW_SOC_MIGRATIONS=YES`, an explicit `JARVIS_SOC_MIGRATION_DATABASE_URL`, and an exact `JARVIS_SOC_MIGRATION_EXPECTED_DATABASE` match. It also verifies each migration checksum from the selected tree, uses a transaction, advisory lock, 2-second lock timeout and 30-second statement timeout. It does not independently attest CT ID/hostname or schema fingerprint; those remain mandatory external prechecks in this runbook. Therefore `PRODUCTION EXECUTION = BLOCKED` until a human-approved window supplies all prechecks and credentials through the protected mechanism. No runner modification is made in this phase.
