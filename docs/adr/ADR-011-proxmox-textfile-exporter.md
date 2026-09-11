# ADR-011: Proxmox guest/service telemetry via node-exporter textfile collector

## Status

Accepted — deployed and verified on 2026-08-11.

## Context

The Core telemetry adapter (`services/core/src/telemetry.rs`) detects down
components with the PromQL query:

    up == 0 or jarvis_proxmox_guest_up == 0 or jarvis_proxmox_service_up == 0

`up` is produced by the existing node-exporter fleet, but
`jarvis_proxmox_guest_up` and `jarvis_proxmox_service_up` had no source. Without
them the query silently degraded to node-exporter reachability only: it could not
tell that a guest was stopped or that a critical service inside a running guest
was inactive.

The Core does **not** expose `/metrics`; it is a Prometheus *consumer*, not a
producer, so scraping the Core for application metrics was not an option.

## Decision

Emit the host-derived gauges from the **Proxmox host** through the node-exporter **textfile
collector**, not through a new standalone exporter.

- `/usr/local/sbin/jarvis-proxmox-vm-metrics` (`deploy/prometheus/jarvis-proxmox-vm-metrics.sh`)
  reads guest state with `pct status` / `qm status` and service state with
  `pct exec <vmid> -- systemctl is-active`, then atomically writes
  `/var/lib/node_exporter/textfile_collector/proxmox_vm_status.prom`.
- For the Ollama LXC, the same collector publishes
  `jarvis_gpu_passthrough_ok{vmid="116"}`. It is `1` only when
  `/dev/dri/renderD129` is a real character device inside the container, so a
  failed or stale GPU bind mount is observable after a host or container boot.
- A oneshot unit + timer (`deploy/systemd/jarvis-proxmox-vm-metrics.{service,timer}`)
  refresh it every 15s.
- The host node-exporter already runs with
  `--collector.textfile.directory=/var/lib/node_exporter/textfile_collector`
  (`deploy/systemd/prometheus-node-exporter-textfile.conf`) and is already
  scraped by Prometheus as the `server-central` target, so the metrics ride on
  that existing scrape.

## Alternatives considered

- **prometheus-pve-exporter + blackbox_exporter**: two community services to
  install and secure. They emit `pve_*` / `probe_success`, which would need
  recording rules to alias into the exact metric names the Core already queries.
  More moving parts and more listening surface for no functional gain.
- **Dedicated custom HTTP exporter**: a new listening port and firewall rule on
  the hypervisor, plus a new Prometheus scrape job. Rejected in favour of the
  textfile collector, which needs neither.

## Consequences

- No new open port, no new firewall rule, no new Prometheus job: the gauges
  appear on the existing `server-central` node-exporter target.
- The metric surface is read-only (`pct status`, `qm status`, `systemctl
  is-active`); it never mutates guests.
- The guest and service inventories are hard-coded in the script. Adding a guest
  or a critical service means editing `GUESTS` / `SERVICES` and redeploying the
  script. This is intentional: the allow-list of what JARVIS watches is explicit.
- Verified 2026-08-11: Prometheus ingests 14 `jarvis_proxmox_guest_up` and 9
  `jarvis_proxmox_service_up` series; the Core down-query correctly reports the
  two intentionally-stopped guests (`dc`, `freeipa`) and nothing else.
