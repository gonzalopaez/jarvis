# SOC v0.2 Backward Compatibility

## Static result

The SQL is additive: all new `soc_cases` columns are nullable without defaults, old columns remain present, and new tables have no triggers affecting legacy writes. Static compatibility is therefore expected for create/update/dedup, 30-minute grouping, legacy priority and `case_events`.

## Runtime result

| COMBINATION | RESULT | REASON |
|---|---|---|
| Old Core + baseline DB | baseline code tests PASS; DB behavioral rehearsal incomplete | baseline schema restored, but synthetic legacy operations were not completed |
| Old Core + new DB | UNVERIFIED | 0001/0002 exact runner execution was blocked |
| New Core + new DB | UNVERIFIED | migrated database was unavailable to the local test binary |

Production gate remains blocked until both runtime combinations pass against a PostgreSQL 15 rehearsal database.
