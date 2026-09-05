# Production preflight — CT133 (read-only until authorization)

This is the final state-validation procedure. It must be run immediately before any separately authorized migration window. Every failure or uncertainty means **ABORT**. Nothing in this document authorizes DDL.

## Git and artifact gate

Run from the reviewed checkout; do not use a dirty Omarchy working tree:

```bash
git fetch origin feature/qdrant-infra-rag
test "$(git rev-parse HEAD)" = "$(git rev-parse origin/feature/qdrant-infra-rag)" || { echo ABORT; exit 1; }
test "$(git status --porcelain)" = "" || { echo ABORT; exit 1; }
test "$(git rev-parse HEAD)" = "7e02f42d2c3ff2e89b357b38bdc085c7f33f1232" || { echo ABORT; exit 1; }
sha256sum services/core/migrations/0001_migration_history.sql services/core/migrations/0002_soc_v02_foundation.sql scripts/soc-migrate.sh
```

Expected hashes are in the runbook. A missing file, branch mismatch or hash mismatch aborts.

## Target identity and PostgreSQL checks

```bash
ssh root@192.168.1.5 'hostname; pveversion; pct status 133; pct config 133'
ssh root@192.168.1.5 'pct exec 133 -- hostname'
ssh root@192.168.1.5 'pct exec 133 -- ip -4 -brief addr'
ssh root@192.168.1.5 'pct exec 133 -- su - postgres -c "psql -d jarvis_soc -X -v ON_ERROR_STOP=1 -Atc \"SELECT current_database(), current_user, version(), current_setting(''server_version'');\""'
```

Require CT ID 133, running state, hostname `jarvis-soc-db`, address `192.168.1.26`, database `jarvis_soc`, and major version 15. Any mismatch aborts.

## Canonical schema fingerprint

Using a protected read-only `psql` invocation, serialize exactly these records with `|` separators and the stated ordering, then hash the resulting file:

```sql
SELECT 'COLUMN|'||table_name||'|'||ordinal_position||'|'||column_name||'|'||data_type||'|'||udt_name||'|'||is_nullable||'|'||coalesce(column_default,'')
FROM information_schema.columns WHERE table_schema='public'
ORDER BY table_name, ordinal_position;
SELECT 'CONSTRAINT|'||conrelid::regclass||'|'||conname||'|'||contype||'|'||pg_get_constraintdef(oid,true)
FROM pg_constraint WHERE connamespace='public'::regnamespace
ORDER BY conrelid::regclass::text, conname;
SELECT 'INDEX|'||tablename||'|'||indexname||'|'||indexdef
FROM pg_indexes WHERE schemaname='public'
ORDER BY tablename, indexname;
```

Require exactly 47 canonical lines and SHA-256 `ba004dd05ecc0bdc8023ef4e7830a65026c4229bb013ae6d3bdd045b97397f5c`. Do not use `pg_dump` textual output as a substitute.

## Migration history, sessions and locks

```sql
SELECT to_regclass('public.jarvis_schema_migrations'); -- must be NULL before 0001
SELECT state, count(*) FROM pg_stat_activity WHERE datname='jarvis_soc' GROUP BY state ORDER BY state;
SELECT pid, usename, state, wait_event_type, wait_event
FROM pg_stat_activity WHERE datname='jarvis_soc' AND state='idle in transaction';
SELECT blocked.pid AS blocked_pid, blocking.pid AS blocking_pid
FROM pg_locks blocked JOIN pg_locks blocking
  ON blocking.locktype=blocked.locktype AND blocking.database IS NOT DISTINCT FROM blocked.database
 AND blocking.relation IS NOT DISTINCT FROM blocked.relation AND blocking.granted
WHERE NOT blocked.granted;
```

Require no migration history, zero `idle in transaction`, and zero blocked sessions. Never terminate a backend automatically.

## Capacity and Core baseline

Record `pg_database_size('jarvis_soc')`, CT filesystem free space, and Proxmox storage free space. Conservative capacity gate: free space must be at least 20% and at least `max(2 × database size, 10 GiB)` so the logical backup and temporary migration/catalog overhead have headroom. If the platform cannot measure either value, abort.

For CT124, record (read-only) `pct status 124`, `systemctl is-active jarvis-core.service`, `systemctl show -p MainPID -p ExecMainStartTimestamp`, health endpoint status if available, and recent error count without printing secrets or query text. Require active/healthy before proceeding.

## Backup validation plan (not executed by preflight)

In the authorized window, create a PostgreSQL custom-format dump on Proxmox backup storage, using protected credentials (not argv):

```bash
pg_dump --format=custom --file=/srv/backup/jarvis_soc_YYYYMMDDTHHMMSSZ.dump "$PROTECTED_LIBPQ_URL"
stat --printf='%s %n\n' /srv/backup/jarvis_soc_*.dump
pg_restore --list /srv/backup/jarvis_soc_YYYYMMDDTHHMMSSZ.dump >/srv/backup/jarvis_soc_YYYYMMDDTHHMMSSZ.list
sha256sum /srv/backup/jarvis_soc_YYYYMMDDTHHMMSSZ.dump
```

The backup is valid only if the command, non-zero size, `pg_restore --list`, and checksum metadata all succeed. Restore to an isolated PostgreSQL 15 target before relying on it. A Proxmox filesystem snapshot/vzdump is secondary protection, not a substitute for a transaction-consistent logical restore path.

## Authorized execution (human approval required; do not run now)

Only after the preflight, backup validation and a separately recorded human approval:

```bash
JARVIS_ALLOW_SOC_MIGRATIONS=YES \
JARVIS_SOC_MIGRATION_EXPECTED_DATABASE=jarvis_soc \
JARVIS_SOC_MIGRATION_DATABASE_URL="$PROTECTED_LIBPQ_URL" \
scripts/soc-migrate.sh
```

The authorization variable must not be exported globally or persisted. Runner failure, checksum mismatch or timeout aborts the window; do not raise the 2-second lock timeout or retry automatically.

## Postchecks and observation

Read-only postchecks verify exactly one history row per migration with approved checksums, all new tables/columns/constraints/indexes, and intact legacy columns. Confirm Core remains active, DB connectivity is healthy, Wazuh ingestion/case grouping/dedup/case_events continue, and no error/lock/session spike occurs. Observe for at least 30 minutes (or the established SOC window) before any later Core v0.2 phase.

## Abort matrix

| Check | Expected | Failure action |
|---|---|---|
| Git/commit | approved HEAD equals origin, clean tree | ABORT |
| Artifact hashes | exact approved SHA-256 | ABORT |
| CT/hostname/IP/DB | 133 / `jarvis-soc-db` / `192.168.1.26` / `jarvis_soc` | ABORT |
| PostgreSQL | major 15 | ABORT |
| Schema fingerprint | 47 lines and approved SHA | ABORT; investigate drift |
| Migration history | absent before 0001 | ABORT |
| Locks/sessions | no blocked or idle-in-transaction | ABORT; do not kill sessions |
| Capacity | 20% and `max(2×DB,10GiB)` free | ABORT |
| Backup/restore | dump validated and restore path available | ABORT |
| Core baseline | active and healthy | ABORT |
| Runner gate | explicit variables and protected URL | ABORT |
| 0001/0002 | success and checksums match | ABORT; restore only by human decision |
| Postchecks/observation | schema and Core healthy | ABORT; stop progression |

