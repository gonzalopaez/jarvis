# Production migration checklist (CT133)

Do not mark ahead of the window. This checklist is operational only; it does not authorize execution.

- [ ] Human change ticket, operator and UTC start recorded
- [ ] Reviewed commit `7e02f42d2c3ff2e89b357b38bdc085c7f33f1232` verified and clean
- [ ] Migration/runner SHA-256 values verified
- [ ] CT133 and hostname `jarvis-soc-db` verified
- [ ] PostgreSQL 15 and database `jarvis_soc` verified
- [ ] Pre-migration schema fingerprint matches rehearsal baseline
- [ ] No unexpected `jarvis_schema_migrations`
- [ ] Disk space, connections, locks and idle transactions acceptable
- [ ] Proxmox backup/snapshot completed
- [ ] Schema/data dump restored and readable on isolated PostgreSQL 15 target
- [ ] Runner safety variables and protected DB credential verified
- [ ] 0001 completed; history/checksum postcheck PASS
- [ ] 0002 completed; schema/legacy-column postcheck PASS
- [ ] Core existing service/health/DB connectivity PASS
- [ ] Ingestion, dedup and case_events smoke checks PASS
- [ ] Observation window completed without error/lock spike
- [ ] Evidence sanitized and change ticket updated
- [ ] Human decision recorded; no automatic Core v0.2 deployment
