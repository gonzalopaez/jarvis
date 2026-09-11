# SOC database migrations

These migrations target the verified CT133 PostgreSQL 15 schema captured in
`docs/discovery/JARVIS_SOC_CT133_SCHEMA_2026-09.sql`.

They are never run by Core at startup. `scripts/soc-migrate.sh` requires an
explicit operator gate, validates database identity, takes a transaction-level
advisory lock, records SHA-256 checksums and refuses changed applied files.

Do not run against production until the exact SQL, locks, backup/restore test,
compatibility and rollback have been approved. Phase 1 prepares these files but
does not apply them.

Lock expectations:

- `0001`: catalog locks for one new table/index.
- `0002`: brief `ACCESS EXCLUSIVE` metadata locks while adding nullable columns;
  catalog locks for new empty tables and their indexes.
- Every transactional file sets a 2-second lock timeout and fails instead of
  waiting behind production traffic.

Rollback is application-first: disable new features and run the previous Core.
Do not drop assessment or feedback history during an incident. The additive
objects may be removed only while empty and under a separately reviewed command.
