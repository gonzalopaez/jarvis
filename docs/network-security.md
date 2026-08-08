# Network security

- AdGuard provides internal DNS names; application configuration does not embed workload IPs.
- Nginx is the internal TLS ingress and uses the existing wildcard certificate.
- Production application traffic uses HTTPS/WSS.
- Privileged service-to-service communication uses mTLS when risk and platform support justify it.
- Every Proxmox VM/LXC receives an explicit least-access firewall policy.
- Management planes and backend ports are not exposed to Desktop or the public Internet.
- Public exposure requires a separate decision, threat review and narrowly scoped firewall/reverse-proxy change.

This repository does not configure AdGuard, Nginx, certificates or firewalls.
