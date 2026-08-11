#!/bin/sh
# JARVIS Proxmox guest/service telemetry for Prometheus.
#
# Runs on the Proxmox host as a oneshot triggered by jarvis-proxmox-vm-metrics.timer.
# Writes a node-exporter textfile collector file with two gauges consumed by the
# Core telemetry adapter (services/core/src/telemetry.rs):
#
#   jarvis_proxmox_guest_up    1 when the guest is running, else 0
#   jarvis_proxmox_service_up  1 when the critical guest service is active, else 0
#
# Read-only: it only calls `pct status` / `qm status` and `systemctl is-active`.
# See docs/adr/ADR-011-proxmox-textfile-exporter.md for the rationale.
set -eu

out=/var/lib/node_exporter/textfile_collector/proxmox_vm_status.prom
tmp="/tmp/jarvis-proxmox-vm-status.$$"

# guest inventory: "vmid|name|kind"
GUESTS="101|adguard|ct 102|opnsense|vm 105|cloudflare-tunnel|ct 106|dc|vm 108|freeipa|vm 109|tailscale-vpn|ct 112|n8n|ct 115|qdrant|ct 116|original-ollama|ct 120|wazuh|ct 123|openbao|ct 124|jarvis-core|ct 125|jarvis-voice|ct 126|jarvis-mcp|ct 127|prometheus|ct"

# critical services: "vmid|name|service". A guest may run more than one service.
SERVICES="101|adguard|AdGuardHome 105|cloudflare-tunnel|cloudflared 109|tailscale-vpn|tailscaled 124|jarvis-core|jarvis-core 124|jarvis-core|jarvis-codex 125|jarvis-voice|jarvis-voice 126|jarvis-mcp|jarvis-mcp 127|prometheus|prometheus 120|wazuh|jarvis-wazuh-relay"

{
  echo "# HELP jarvis_proxmox_guest_up Whether a critical Proxmox guest is running."
  echo "# TYPE jarvis_proxmox_guest_up gauge"
  for entry in $GUESTS; do
    vmid=${entry%%|*}; rest=${entry#*|}; name=${rest%%|*}; kind=${rest##*|}
    if [ "$kind" = vm ]; then status=$(qm status "$vmid" 2>/dev/null | awk "{print \$2}"); else status=$(pct status "$vmid" 2>/dev/null | awk "{print \$2}"); fi
    [ "$status" = running ] && value=1 || value=0
    printf "jarvis_proxmox_guest_up{vmid=\"%s\",name=\"%s\",kind=\"%s\"} %s\\n" "$vmid" "$name" "$kind" "$value"
  done
  echo "# HELP jarvis_proxmox_service_up Whether a critical guest service is active."
  echo "# TYPE jarvis_proxmox_service_up gauge"
  for entry in $SERVICES; do
    vmid=${entry%%|*}; rest=${entry#*|}; name=${rest%%|*}; service=${rest##*|}
    if pct status "$vmid" 2>/dev/null | grep -q running && pct exec "$vmid" -- systemctl is-active --quiet "$service" 2>/dev/null; then value=1; else value=0; fi
    printf "jarvis_proxmox_service_up{vmid=\"%s\",name=\"%s\",service=\"%s\"} %s\\n" "$vmid" "$name" "$service" "$value"
  done
} > "$tmp"

chown root:prometheus "$tmp"
chmod 0644 "$tmp"
mv -f "$tmp" "$out"
