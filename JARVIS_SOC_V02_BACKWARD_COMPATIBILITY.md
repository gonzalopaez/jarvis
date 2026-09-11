# SOC v0.2 Backward Compatibility

## Runtime evidence (Fase 1.6)

The guarded integration harness ran against PostgreSQL 15.19 in temporary CT134, database `jarvis_soc_integration_nonprod`, from the published checkout. Six tests passed: create/update, alert-id deduplication, 30-minute grouping, legacy priority behavior, fixed timestamp/MITRE persistence, malformed identifier rejection, and assessment projection/rollback paths.

| COMBINATION | RESULT | SCOPE |
|---|---|---|
| Old Core + new DB | PASS | Legacy `SocCaseStore::ingest` contract exercised against migrated schema; exact historical binary was not deployed. |
| New Core + new DB | PASS | Current Core DB persistence path and guarded assessment APIs exercised; no production Core was used. |

All legacy columns (`priority`, `confidence`, `mitre_techniques`, `assigned_to`, `status`, `case_events`) remained usable. The upstream Wazuh poller/EventBus process is covered by existing unit/fixture suites; this gate specifically proves the PostgreSQL runtime boundary.
