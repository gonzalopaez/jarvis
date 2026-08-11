# Network security

- AdGuard provides internal DNS names; application configuration does not embed workload IPs.
- Nginx is the internal TLS ingress and uses the existing wildcard certificate.
- Production application traffic uses HTTPS/WSS.
- Privileged service-to-service communication uses mTLS when risk and platform support justify it.
- Every Proxmox VM/LXC receives an explicit least-access firewall policy.
- Management planes and backend ports are not exposed to Desktop or the public Internet.
- Public exposure requires a separate decision, threat review and narrowly scoped firewall/reverse-proxy change.

The repository carries the reviewed Core ingress fragment, but production AdGuard, certificate and firewall state remains operational configuration outside Git.

The initial Core deployment uses the internal name `jarvis.d4rkn0d3.com`. AdGuard resolves it to the Nginx Proxy Manager workload, which terminates TLS with the existing wildcard certificate and forwards only to Core's private listener. Core's guest firewall accepts that application port only from Nginx Proxy Manager.

Jarvis Core code additionally denies binding to unspecified or public addresses. The supported boundary is loopback, RFC1918 IPv4 or IPv6 unique-local. This is defense in depth and does not replace VM/LXC firewall rules or Nginx TLS policy.
