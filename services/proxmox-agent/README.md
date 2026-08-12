# Proxmox Agent

Proposal-only Python domain agent for `proxmox.vm.deploy`,
`proxmox.vm.destroy`, and `proxmox.ct.destroy`. It has no Proxmox API client or
credential. Every tool submits `kind="action"` to Core and stops at centralized
authorization. Core calls have an 8-second timeout, bounded response, and 12 KiB
parameter limit.
