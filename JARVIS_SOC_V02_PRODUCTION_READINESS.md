# SOC v0.2 Production Readiness

| GATE | RESULT |
|---|---|
| MIGRATIONS | PASS (NONPROD) |
| OLD CORE + NEW DB | PASS (legacy store contract harness) |
| NEW CORE + NEW DB | PASS (guarded Core DB boundary) |
| MITRE END-TO-END | PASS (synthetic IDs verified in PostgreSQL) |
| RISK ENGINE | PASS |
| CONFIDENCE ENGINE | PASS |
| ASSESSMENT PERSISTENCE | PASS (append-only projection + rollback) |
| AI / ANALYST SEPARATION | PASS |
| 90/90 INTERNAL | PASS |
| PRODUCTION DDL | APPROVED_FOR_REVIEW |

## Why

PostgreSQL 15 baseline restoration, exact-byte runner execution, and the guarded six-test runtime harness succeeded in temporary CT134. “Old Core” denotes the legacy `SocCaseStore` interface contract; no production binary was deployed. Approval is for human review only, never execution.

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
