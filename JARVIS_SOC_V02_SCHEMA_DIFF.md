# SOC v0.2 Schema Diff

This is the reviewed logical diff between the verified CT133 baseline and migration 0002. It is **not** a post-execution fingerprint because the exact rehearsal execution was blocked.

| CHANGE | CLASS | COMPATIBILITY | RISK |
|---|---|---|---|
| Add 20 nullable projection/SLA/verdict columns to `soc_cases` | ADDITIVE | old Core ignores them | brief ACCESS EXCLUSIVE metadata lock |
| Add eight `NOT VALID` checks to `soc_cases` | ADDITIVE | legacy rows are not scanned; new writes are checked | brief ACCESS EXCLUSIVE lock |
| Create `soc_assessments`, identity and index | ADDITIVE / REQUIRES_NEW_CORE | old Core unaffected | referenced-case FK; no cascade |
| Create `soc_feedback`, identity and index | ADDITIVE / REQUIRES_NEW_CORE | old Core unaffected | referenced history FKs; no cascade |
| Create `jarvis_schema_migrations` in 0001 | ADDITIVE | application-independent | catalog lock only |

No reviewed statement contains `DROP`, destructive `RENAME`, type narrowing, backfill, or deletion. Existing `soc_cases.priority`, `confidence`, `mitre_techniques`, `assigned_to`, `alert_ids` and all other baseline columns are untouched.

Final schema fingerprint A/B: **UNVERIFIED**. Migration durations and observed lock waits: **UNVERIFIED**.
