# SOC v0.2 Schema Diff

This is the reviewed diff between the verified CT133 baseline and migration 0002, validated against PostgreSQL 15 rehearsal databases A/B from the published commit.

| CHANGE | CLASS | COMPATIBILITY | RISK |
|---|---|---|---|
| Add 20 nullable projection/SLA/verdict columns to `soc_cases` | ADDITIVE | old Core ignores them | brief ACCESS EXCLUSIVE metadata lock |
| Add eight `NOT VALID` checks to `soc_cases` | ADDITIVE | legacy rows are not scanned; new writes are checked | brief ACCESS EXCLUSIVE lock |
| Create `soc_assessments`, identity and index | ADDITIVE / REQUIRES_NEW_CORE | old Core unaffected | referenced-case FK; no cascade |
| Create `soc_feedback`, identity and index | ADDITIVE / REQUIRES_NEW_CORE | old Core unaffected | referenced history FKs; no cascade |
| Create `jarvis_schema_migrations` in 0001 | ADDITIVE | application-independent | catalog lock only |

No reviewed statement contains `DROP`, destructive `RENAME`, type narrowing, backfill, or deletion. Existing `soc_cases.priority`, `confidence`, `mitre_techniques`, `assigned_to`, `alert_ids` and all other baseline columns are untouched.

Canonical post-migration A/B fingerprint (excluding random pg_dump guard lines): `a4f80f8566ad9958ad7b4948df4eb32b3c7807e20020f2e4d9b9b558a9e30100` for both databases. No DROP, destructive rename, type narrowing or legacy constraint removal was observed. 0002 completed in approximately 0.55 s on these small databases; lock timeout occurred at approximately 2.55 s.
