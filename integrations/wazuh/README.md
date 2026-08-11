# Wazuh integration

## Read-only security telemetry (live)

Normalized Wazuh alerts are served by the token-protected relay
(`services/wazuh-relay`, CT120 / 192.168.1.10:5515) and consumed read-only by:

- the Core, which answers governed security questions ("are there critical
  alerts?", "show me the Wazuh alerts", "what happened on DC?") from this evidence;
- the MCP gateway tool `wazuh.security.alerts`, filtered by endpoint and severity.

The relay requires a bearer token (unauthenticated requests get 401). No alert
consumer can modify Wazuh.

## Restricted response actions (future)

Automated mitigations, blocking users/IPs and any write path into Wazuh remain
future work behind policy and human authorization. The deployed
`RestrictedExecutor` is intentionally disabled.
