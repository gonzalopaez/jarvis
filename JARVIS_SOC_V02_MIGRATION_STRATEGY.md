# SOC v0.2 Migration Strategy

Status: proposed mechanism; no migration created or executed.

## Minimal mechanism

Use ordered plain PostgreSQL SQL files plus a small explicit runner or documented `psql` procedure. Do not auto-migrate during Core boot.

Tentative layout after live schema validation:

```text
services/core/migrations/
  README.md
  0001_baseline_verified_schema.sql
  0002_soc_assessment_history.sql
  0003_soc_case_operational_fields.sql
  0004_soc_feedback_history.sql
  rollback/
    0004_soc_feedback_history.sql
```

`0001` should be a non-applying baseline/check artifact describing expected live object signatures, not recreate production tables.

## Migration history

If CT133 has no existing mechanism, introduce `jarvis_schema_migrations` in the application schema with:

- immutable migration version/name;
- SHA-256 checksum of applied SQL;
- applied timestamp;
- operator/tool version;
- success is represented only by a committed row in the same transaction.

The runner takes an advisory lock, validates checksums/order, refuses unknown or modified applied files and runs one transactional migration at a time. It requires explicit invocation and a dedicated DDL-authorized maintenance identity. Core runtime credentials should remain DML-limited.

## SQL rules

- Forward-only is the default for additive production schema.
- `ADD COLUMN` starts nullable and without volatile defaults; backfill is a separately reviewed migration if required.
- Add constraints as `NOT VALID` where supported, validate separately after checking data, then tighten only in a future phase.
- Use `CREATE TABLE IF NOT EXISTS` only with an explicit structural assertion; silent name collision is not acceptable idempotence.
- Use `CREATE INDEX CONCURRENTLY` outside transaction for large live tables, with its own migration state and recovery instructions.
- Set conservative `lock_timeout` and `statement_timeout`; abort on contention rather than blocking case ingestion.
- Never include secrets, owners tied to unknown environments, production data or destructive cleanup.

## Precondition and backup plan

Before showing final SQL for approval:

1. Capture sanitized schema-only dump and catalog inventory.
2. Record PostgreSQL version, database/schema/search path, owners, grants, table sizes and active dependencies.
3. Identify current migration history mechanism.
4. Obtain a production backup/snapshot through the existing infrastructure process and verify restoration in a non-production target. This assistant does not create the backup without explicit operational access/authorization.
5. Restore schema/data copy in staging and run old Core tests/queries before and after migration.
6. Show exact SQL, objects, expected locks, compatibility and rollback to the operator.
7. Apply manually in a controlled window; do not restart services unless separately approved.

## Expected locks

- `CREATE TABLE`: catalog locks; no existing table rewrite.
- `ALTER TABLE ... ADD COLUMN` nullable/no default: brief `ACCESS EXCLUSIVE` metadata lock on PostgreSQL 15; schedule and enforce low lock timeout.
- `ADD CONSTRAINT NOT VALID`: brief table lock; validation later uses weaker locks depending on constraint.
- `CREATE INDEX CONCURRENTLY`: avoids blocking normal writes but performs multiple scans and waits for transactions; monitor load and failure residue.

These are generic PostgreSQL expectations. Actual lock risk cannot be assessed until table sizes, traffic and live definitions are known.

## Backward compatibility

- Old Core continues to use existing columns and tables.
- Keep current `priority`, `case_events.evidence`, status strings and flat Wazuh fields through v0.2 rollout.
- New readers tolerate null v0.2 columns and absent assessments.
- New writers dual-populate projections only after schema capability detection/version gate.
- Feature flags remain off until migration verification.

## Rollback

- Preferred rollback is application/flag rollback while leaving additive unused schema in place.
- Do not drop evidence/history during an incident rollback.
- Down scripts are supplied only for empty/unreferenced new objects and require explicit row-count/precondition checks.
- Failed concurrent indexes may be dropped only after exact-name verification and operator approval.
- A checksum/history row is never manually edited to disguise failure.

## Proposed commits

1. `docs(soc): capture phase 0.5 read-only validation`
2. `chore(db): add explicit SOC migration runner and verified baseline`
3. `feat(wazuh): add canonical normalized event contract`
4. `feat(wazuh): preserve Wazuh MITRE and entities`
5. `feat(soc): add append-only assessment persistence`
6. `feat(soc): add deterministic risk engine v1`
7. `feat(soc): add deterministic confidence engine v1`
8. `test(soc): add anonymized foundation fixtures`

No automatic commits are performed.

## Current decision

CT133 schema is now verified and has no migration history. Its three small existing tables do not conflict destructively with an additive assessment/history design. **Phase 1 is unblocked for local implementation only.** No migration may be applied to production until its exact SQL, locks, backup preconditions, compatibility and rollback are presented and approved.
