#!/bin/sh
# Proxmox hookscript for CT 116. Wait for the AMD DRM nodes before LXC setup so
# its native lxc.mount.entry directives cannot silently bind missing sources.
set -eu

vmid=${1:?missing vmid}
phase=${2:?missing hook phase}

[ "$vmid" = 116 ] || exit 0
devices="/dev/kfd /dev/dri/card1 /dev/dri/renderD129"
log_event() {
  message=$1
  logger -t jarvis-gpu-hook "$message"
  printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$message" >> /var/log/jarvis-gpu-hook.log
}

if [ "$phase" = post-start ]; then
  exit 0
fi
[ "$phase" = pre-start ] || exit 0

attempt=0
while :; do
  ready=1
  for device in $devices; do
    [ -c "$device" ] || ready=0
  done
  [ "$ready" -eq 1 ] && break
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 120 ]; then
    log_event "vmid=116 result=failed reason=host_devices_timeout"
    exit 1
  fi
  sleep 1
done

kfd_major_hex=$(stat -c '%t' /dev/kfd)
kfd_major=$((0x$kfd_major_hex))
if [ "$kfd_major" -ne 511 ]; then
  log_event "vmid=116 result=failed reason=kfd_major_mismatch actual=$kfd_major expected=511"
  exit 1
fi

log_event "vmid=116 phase=pre-start result=ok kfd_major=$kfd_major"
