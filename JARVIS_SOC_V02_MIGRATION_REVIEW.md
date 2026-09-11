# SOC v0.2 Migration Review

Reviewed: 2026-09-04, before non-production execution.

## Findings corrected before rehearsal

1. Runner hardcoded database name `jarvis_soc`; changed to require an explicit exact `JARVIS_SOC_MIGRATION_EXPECTED_DATABASE`, allowing an unmistakable rehearsal database.
2. Migration-history lookup suppressed every SQL error; changed to query `to_regclass` first and fail on real query/connection errors.
3. Assessment/feedback FKs used `ON DELETE CASCADE`; removed cascade so case deletion cannot silently erase assessment or analyst history.
4. Added a check to `previous_analyst_verdict`.
5. Added an optional explicit migration root so failure/checksum tests can use temporary copies without editing approved migrations.

## Statement review

| STATEMENT | OBJECT | LOCK EXPECTED | OLD CORE COMPATIBILITY | IMPACT / ROLLBACK | PRECONDITION / SECURITY |
|---|---|---|---|---|---|
| `SET LOCAL lock_timeout='2s'` | transaction | none | transparent | aborts quickly on contention | must run inside runner transaction |
| `SET LOCAL statement_timeout='30s'` | transaction | none | transparent | aborts long statement | must run inside runner transaction |
| create migration table | `jarvis_schema_migrations` | catalog + new relation locks | transparent | leave unused on app rollback | name absent in verified baseline |
| migration PK/check/defaults | history table | new-table only | transparent | protects order/checksum metadata | runner records only after file succeeds |
| `ADD COLUMN` nullable | `soc_cases` | brief ACCESS EXCLUSIVE metadata lock | compatible | old Core ignores new columns; app rollback leaves them | verified names absent; 2s lock timeout |
| add case CHECK NOT VALID | `soc_cases` | brief ACCESS EXCLUSIVE | compatible | enforced for new/changed rows; validation deferred | legacy rows not scanned during add |
| create assessments | `soc_assessments` | catalog/new-table locks | compatible | append-only history retained | case PK verified bigint |
| assessment case FK | assessments/cases | locks for FK definition on new table | compatible | default NO ACTION preserves history | no cascade |
| assessment self-FK | assessments | new-table lock | compatible | L2 may reference prior L1 | nullable |
| assessment index | new table | new-table lock | compatible | removable only if unused | no production table scan |
| create feedback | `soc_feedback` | catalog/new-table locks | compatible | history retained on app rollback | verdict checks separate from AI |
| feedback FKs/index | feedback/case/assessment | new-table + referenced metadata locks | compatible | default NO ACTION | no cascade |
| advisory xact lock | DB advisory namespace `12413302` | transaction advisory lock | transparent | auto-released commit/rollback/session loss | serializes this migration runner only |
| migration transaction | one file per transaction | locks held until commit | transparent after commit | all statements/history row roll back together | `ON_ERROR_STOP=1` |
| checksum lookup | history table | ACCESS SHARE | transparent | changed applied file rejected | SHA-256 calculated locally |

## Nullable/default review

All new `soc_cases` columns are nullable and have no backfill/default, avoiding a table rewrite and preserving old inserts. New-table defaults are immutable constants or `now()` evaluated on insert. JSON arrays default to empty JSON arrays only in newly created assessment tables.

## Remaining review notes

- Migration SQL has not yet been executed on production.
- SQL must be rehearsed against PostgreSQL 15 and schema fingerprints compared twice.
- Production credentials should be supplied through an existing secure libpq mechanism; never embed passwords in command history.
- Operational rollback is previous application + feature flags; history tables are not dropped.
