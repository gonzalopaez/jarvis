# Core deployment

Deployment artifacts contain no production addresses or credentials.

- Create `/etc/jarvis-core/environment` as root-only operational configuration with `JARVIS_CORE_BIND=<private-address>:4100` and `JARVIS_WEB_ORIGIN=https://<internal-jarvis-dns-name>`. The origin must exactly match the browser `Origin` header and cannot contain a path or trailing slash.
- Render `nginx/jarvis-core.conf.template` by replacing `{{JARVIS_CORE_UPSTREAM}}` with the same private address before installing it in Nginx Proxy Manager.
- Install `nginx/jarvis-security-headers.conf` as `/etc/nginx/snippets/jarvis-security-headers.conf`. It is included again in static locations because Nginx does not inherit parent `add_header` directives when a location defines its own headers.
- Build `apps/desktop` with `npm ci && npm run build` and install the generated `dist/` contents at `/usr/share/jarvis/web` on the Nginx workload. The browser receives static assets only; it does not receive Core credentials.
- Provide `auth-registry.json` through the systemd `LoadCredential` source configured by the service unit. The registry contains only credential digests and server-owned identities; raw Bearer values remain outside Git.

Validate the rendered Nginx configuration before reload and ensure the workload firewall permits port 4100 only from the proxy workload.

## Codex expert service

The Core accepts an optional `JARVIS_CODEX_URL`. Keep it unset until the
Codex service has a dedicated service token and a LiteLLM virtual key
provisioned outside Git (preferably through OpenBao). Do not copy a personal `~/.codex`
login from an endpoint into the server.

The service unit binds to loopback on CT 124 and uses `read-only` sandboxing,
disabled web search/MCP, bounded task concurrency and a bounded task timeout.
Install its built `services/codex-service` directory at `/opt/jarvis-codex`,
provide the two systemd credentials referenced by
`deploy/systemd/jarvis-codex.service`, then set:

```text
JARVIS_CODEX_URL=http://127.0.0.1:4400/
```

in `/etc/jarvis-core/environment` and add the corresponding Core
`LoadCredential=codex-service-token` entry. Restart Codex Service first,
verify its private health endpoint, then restart Core and verify
`/api/v1/health` reports `CODEX AGENT` as `READY`.

The Phase 1 browser runtime can use the public minimal health route. Authenticated browser commands intentionally remain unavailable until the versioned API provides secure server-side sessions; do not inject a shared Bearer credential in Nginx or frontend code.
