# SOC v0.2 Production Readiness

| GATE | RESULT |
|---|---|
| MIGRATIONS | PASS (NONPROD) |
| OLD CORE + NEW DB | FAIL (not demonstrated) |
| NEW CORE + NEW DB | FAIL (not demonstrated) |
| MITRE END-TO-END | FAIL (not demonstrated through Core DB path) |
| RISK ENGINE | PASS |
| CONFIDENCE ENGINE | PASS |
| ASSESSMENT PERSISTENCE | FAIL (DB runtime not exercised) |
| PRODUCTION DDL | BLOCKED |

## Why

PostgreSQL 15 baseline restoration and exact-byte runner execution succeeded inside CT134. Core runtime integration remains unverified because production Core was correctly not deployed and CT134 lacks a Rust toolchain. Absence of evidence is not treated as success.

## Required review sequence

1. Create a PostgreSQL 15 NONPROD database from the schema-only baseline.
2. Supply a secure libpq connection without embedding a password in shell history.
3. Run `scripts/soc-migrate.sh` with `JARVIS_ALLOW_SOC_MIGRATIONS=YES`, exact `JARVIS_SOC_MIGRATION_DATABASE_URL`, and exact `JARVIS_SOC_MIGRATION_EXPECTED_DATABASE`.
4. Execute rerun, checksum-copy, deliberate-failure and 2-second lock-timeout tests.
5. Repeat from a fresh second database and compare canonical schema-only fingerprints.
6. Execute synthetic legacy behavior before and after migration.
7. Execute new Core canonical/MITRE/timestamp/null/assessment integration tests.
8. Record durations, locks and fingerprints in the rehearsal results.
9. Only after every gate passes, classify DDL as `APPROVED_FOR_REVIEW`; a separate human approval is still required for production execution.

Rollback is application rollback plus feature flags while leaving additive columns/history tables in place. No destructive down migration is proposed.
