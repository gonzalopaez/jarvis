# SOC v0.2 Non-Production Migration Rehearsal

Date: 2026-09-04. Production was not changed.

## Environment and completed checks

- Created temporary unprivileged CT134 `jarvis-soc-rehearsal` on Proxmox, explicitly `onboot=0`, 1 vCPU, 1 GiB RAM and 4 GiB disk.
- Installed PostgreSQL **15.19** inside CT134 only.
- Created the unmistakably non-production database `jarvis_soc_rehearsal_a`.
- Restored `docs/discovery/JARVIS_SOC_CT133_SCHEMA_2026-09.sql` successfully with `ON_ERROR_STOP=1`. PostgreSQL reported creation of the three baseline tables, their two sequences, constraints and three indexes without errors.
- No production data was copied; the input was schema-only.
- Destroyed CT134 and its dedicated logical volume after the blocked rehearsal. It is not recoverable and contained no production data.

## Resumed rehearsal from published commit

Proxmox checked out `733e5e1a53aaf8b8660371d4cac524ece161ab23` detached and clean. Exact SHA-256 values matched local values:

| FILE | SHA-256 |
|---|---|
| 0001 | `9509aab1a0a5913a983dd119a4ac14f1871d883576b75455b5b9b5a574a5fb3b` |
| 0002 | `6de4fccf0f0f8d80e0a59197b46b65b4cd2ad45864fc0f0cc544f901e28ef294` |
| runner | `89ad7394bad588fbe55b40a21cc0b96a2c85a6731bfcd301250e6d4bff5c70ad` |
| baseline | `a54a3d5d5ea801e0b6860902ea35314a685de31b5d7c80ca4d9992e77243a137` |
| event schema | `034ca04158075a0cd378a80cc73e8793350db3b3740e026470137ba4c3ade0bb` |

Baseline restoration succeeded in databases A/B. The exact runner applied 0001/0002 to both; complete rerun was a safe NOOP. Checksum mutation was rejected with exit 3. Deliberately invalid `9999_test_failure.sql` returned exit 3 and recorded zero success rows. A conflict lock caused `lock_timeout` at approximately 2.55 s with no partial schema; after release, migration completed in approximately 0.54 s. Canonical schema fingerprints A/B matched after removing only random pg_dump `\\restrict` guards: `a4f80f8566ad9958ad7b4948df4eb32b3c7807e20020f2e4d9b9b558a9e30100`.

## Blocking condition

`STDIN_TRANSFER = BLOCKED`. The required inocuous probe (`printf ... | ssh ... 'cat > /tmp/... && sha256sum ...'`) was rejected by the managed execution environment before SSH session establishment with `socket: Operation not permitted`. Per procedure, no transfer workaround was attempted.

The earlier managed environment blocked Omarchy-originated stdin transfer and tunnels. This resumed run used Git on Proxmox and `pct push` from the Proxmox checkout into CT134; no Omarchy runtime was used.

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

The remaining unverified items are Core runtime integration, MITRE-to-PostgreSQL through `SocCaseStore`, assessment transaction failure injection and L1/L2 history. CT134 has no Rust toolchain and no production Core binary was deployed.

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
