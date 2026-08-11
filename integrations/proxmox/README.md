# Proxmox integration

## Read-only telemetry (live)

Proxmox guest and service state is exported to Prometheus from the Proxmox host
via the node-exporter textfile collector and consumed read-only by the Core
telemetry adapter and the MCP gateway:

- `deploy/prometheus/jarvis-proxmox-vm-metrics.sh` emits `jarvis_proxmox_guest_up`
  and `jarvis_proxmox_service_up` (see
  [ADR-011](../../docs/adr/ADR-011-proxmox-textfile-exporter.md)).
- The MCP gateway exposes allow-listed, schema-bound read tools
  (`proxmox.vm.list`, `proxmox.vm.status`) scoped to the `JARVIS` pool
  (CT124, CT125) using a dedicated Proxmox API token with read scope.

These paths only observe (`pct status`, `qm status`, `systemctl is-active`, the
Proxmox read API). They never mutate guests.

## Restricted write adapter (future)

A capability-scoped adapter for infrastructure actions (start/stop/restart, etc.)
behind policy, human authorization and single-use grants remains future work. The
deployed `RestrictedExecutor` is intentionally disabled.
