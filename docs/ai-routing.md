# Hybrid AI routing

JARVIS separates model routing from agent routing. `CapabilityRouter` receives normalized text or voice requests and emits a structured `RoutingDecision`; it never executes a tool.

| Route | Target | Purpose |
|---|---|---|
| `FAST_MODEL` | LiteLLM `jarvis-fast` | Low-latency conversation |
| `REASONING_MODEL` | LiteLLM `jarvis-reasoning` | General complex reasoning |
| `CODEX` | Codex Service | Technical expert work |
| `INFRASTRUCTURE_AGENT` | Infrastructure Agent | Infrastructure diagnosis through MCP |
| `SECURITY_AGENT` | Security Agent | SOC/security analysis |
| `AUTOMATION` | n8n | Scheduled workflows and integrations |
| `MCP_TOOL` | MCP Gateway | Explicit structured capabilities only |

`AUTO` is the default. `FAST`, `SMART`, and `EXPERT` are contract-level overrides for future authenticated controls. The first deterministic classifier is centralized and covered by tests; it can later be complemented by a model classifier without changing downstream contracts.

Codex unavailability never adds latency to `FAST_MODEL`. An expert request fails explicitly when Codex is unavailable; it is not presented as expert analysis from another component.
