# SOC v0.2 Non-Production Migration Rehearsal

Date: 2026-09-04. Production was not changed.

## Environment and completed checks

- Created temporary unprivileged CT134 `jarvis-soc-rehearsal` on Proxmox, explicitly `onboot=0`, 1 vCPU, 1 GiB RAM and 4 GiB disk.
- Installed PostgreSQL **15.19** inside CT134 only.
- Created the unmistakably non-production database `jarvis_soc_rehearsal_a`.
- Restored `docs/discovery/JARVIS_SOC_CT133_SCHEMA_2026-09.sql` successfully with `ON_ERROR_STOP=1`. PostgreSQL reported creation of the three baseline tables, their two sequences, constraints and three indexes without errors.
- No production data was copied; the input was schema-only.
- Destroyed CT134 and its dedicated logical volume after the blocked rehearsal. It is not recoverable and contained no production data.

## Blocking condition

`STDIN_TRANSFER = BLOCKED`. The required inocuous probe (`printf ... | ssh ... 'cat > /tmp/... && sha256sum ...'`) was rejected by the managed execution environment before SSH session establishment with `socket: Operation not permitted`. Per procedure, no transfer workaround was attempted.

The managed execution environment allowed read-only SSH and lifecycle commands but rejected every file transfer and SSH tunnel attempt with `socket: Operation not permitted`. This prevented the exact local `scripts/soc-migrate.sh` and reviewed migration bytes from reaching or connecting to CT134. Inline replacement was deliberately not treated as equivalent evidence.

### Manual byte-exact procedure (operator-run only)

From the repository root, an authorized operator may run the following with the existing SSH identity and known-host policy. Each command streams the local bytes and verifies the remote SHA-256 before any migration is started:

```bash
PROXMOX='root@192.168.1.5'
SSH='ssh -F /dev/null -o BatchMode=yes -o IdentitiesOnly=yes -o StrictHostKeyChecking=yes -o UserKnownHostsFile=/home/d4rkn0d3/.ssh/known_hosts -i /home/d4rkn0d3/.ssh/id_ed25519_jarvis_proxmox'
$SSH "$PROXMOX" 'mkdir -p /tmp/jarvis-soc-v02-rehearsal'
for f in services/core/migrations/0001_migration_history.sql services/core/migrations/0002_soc_v02_foundation.sql scripts/soc-migrate.sh docs/discovery/JARVIS_SOC_CT133_SCHEMA_2026-09.sql; do
  n="/tmp/jarvis-soc-v02-rehearsal/$(basename "$f")"
  sha256sum "$f"
  cat "$f" | $SSH "$PROXMOX" "cat > '$n'"
  $SSH "$PROXMOX" "sha256sum '$n'"
done
$SSH "$PROXMOX" 'chmod 700 /tmp/jarvis-soc-v02-rehearsal/soc-migrate.sh'
```

Proceed only when every local and remote digest is identical and the target database name contains `rehearsal` or `nonprod`; never point the runner at CT133.

Consequently, the following remain **NOT EXECUTED / UNVERIFIED**: 0001/0002 execution, rerun, checksum mismatch, failure rollback, advisory/DDL lock, two-database fingerprint comparison, invalid constraint inputs, old-Core SQL behavior on the migrated schema, and runtime assessment persistence against PostgreSQL.

## Local verification completed

- Rust workspace: PASS (all reported test binaries green; 79 Rust tests total in this workspace invocation).
- Clippy workspace/all targets with warnings denied: PASS.
- Frontend: 27/27 PASS; production build PASS.
- Python: 20/20 PASS when run from each service's required working directory.
- canonical event JSON Schema parse: PASS.
- migration runner shell syntax: PASS.
- secret scan: clean; only existing private-IP advisories.
- Risk determinism now evaluates the identical input 100 times.
- Pure priority and 90/90 candidate tests: PASS; no WebSocket, voice or Tier 2 connection exists.
- Duplicate Wazuh adapter observations with the same Wazuh alert ID count as one source: PASS.

## Production impact

CT133, CT124, n8n, LiteLLM, Wazuh and RestrictedExecutor were untouched. No production migration, deployment, restart, workflow, quarantine or Tier 2 action occurred.
