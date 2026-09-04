# SOC v0.2 Backward Compatibility

## Static result

The SQL is additive: all new `soc_cases` columns are nullable without defaults, old columns remain present, and new tables have no triggers affecting legacy writes. Static compatibility is therefore expected for create/update/dedup, 30-minute grouping, legacy priority and `case_events`.

## Runtime result

| COMBINATION | RESULT | REASON |
|---|---|---|
| Old Core + baseline DB | baseline code tests PASS; DB behavioral rehearsal incomplete | baseline schema restored, but synthetic legacy operations were not completed |
| Old Core + new DB | UNVERIFIED | migrations pass; CT134 has no Rust/toolchain harness |
| New Core + new DB | UNVERIFIED | migrated database was unavailable to the local test binary |

Schema-level compatibility passes: legacy columns/constraints survived and synthetic legacy rows were accepted. Runtime gate remains blocked until both combinations pass with an executable Core harness.
