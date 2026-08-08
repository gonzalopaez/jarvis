# Proxmox architecture

Future JARVIS workloads run in purpose-scoped VMs/LXCs. Each workload has a defined owner, trust level, network zone, firewall policy, update strategy, backup policy and least-privilege service identity.

JARVIS Desktop never connects to Proxmox. Infrastructure operations pass through Jarvis Core, policy, authorization and a restricted infrastructure executor. No Proxmox deployment or configuration change is part of v0.1-clean.
