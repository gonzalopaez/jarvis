# Wazuh Agent

Python domain agent for ADR-014. It reads and normalizes local Wazuh alerts,
uses the `jarvis-soc-l1` and `jarvis-soc-l2` LiteLLM aliases for structured
triage, and submits proposed containment actions to Core as `kind="action"`
requests.

The agent has no FreeIPA or Wazuh Active Response client and cannot execute a
proposal. Its action allow-list is limited to `security.user.disable`,
`security.ip.block`, and `security.host.isolate`. Core remains the policy and
authorization boundary, and the production RestrictedExecutor remains
disabled.

Every LiteLLM request has an 8-second deadline and a 12 KiB maximum evidence
context. Upstream responses and inbound HTTP requests are also bounded.

The default HTTP mode preserves the former relay response on port 5515 so Core
and the MCP gateway can continue reading alerts. `--mcp` exposes
`wazuh.alerts.read` and `wazuh.alert.triage` over stdio MCP.

Run the test suite from the repository root:

```bash
PYTHONPATH=services/wazuh-agent python3 -m unittest discover \
  -s services/wazuh-agent -p 'test_*.py' -v
```
