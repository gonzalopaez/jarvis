# Wazuh integration

## Security evidence and proposals (live)

Normalized Wazuh alerts are served by the token-protected Wazuh Agent
(`services/wazuh-agent`, CT120 / port 5515) and consumed read-only by:

- the Core, which answers governed security questions ("are there critical
  alerts?", "show me the Wazuh alerts", "what happened on DC?") from this evidence;
- the MCP gateway tool `wazuh.security.alerts`, filtered by endpoint and severity.

The agent performs structured L1/L2 triage through LiteLLM and may submit only
the three allow-listed security proposals to Core. It has no direct FreeIPA or
Wazuh Active Response write path. Core remains the authorization boundary and
the production RestrictedExecutor remains disabled.

## n8n boundary

The production n8n `SOC 2.0` workflow only normalizes, correlates in a bounded
five-minute window, and notifies. It contains no LLM or action nodes.
