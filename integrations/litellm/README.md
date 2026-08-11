# LiteLLM integration

LiteLLM is the model control plane (ADR-002): Jarvis Core and the n8n SOC
workflow are its clients, and physical model choices live in `config.yaml`, not
in caller code.

## What is active

- **Conversation routing** — `services/core/src/voice.rs` calls LiteLLM through
  named aliases (`jarvis-fast`, `jarvis-reasoning`) rather than physical model
  names. This path is live.
- **SOC triage aliases** — `jarvis-soc-l1` and `jarvis-soc-l2` (ADR-012) are the
  new addition, consumed by the n8n "SOC 2.0" workflow for L1/L2 alert triage.
  They require structured output against
  `contracts/api/security-verdict.v1.schema.json`, replacing the workflow's
  previous free-text regex parsing.

## What remains future

- MCP and Agent Gateway routing *through* LiteLLM (ADR-008) is not wired yet.
- Forwarding a verdict's `proposed_actions` to Core's action endpoint is a
  separate, not-yet-implemented contract; the RestrictedExecutor stays disabled.

## config.yaml

`config.yaml` is a **sanitized reference**: no real keys, no hosts beyond the
documented internal addresses. The master key, database URL and per-consumer
virtual keys are provisioned through environment / OpenBao and issued via
LiteLLM's `/key/generate` API — never committed here. Each consumer
(`core-conversation`, `n8n-soc-triage`) gets its own budgeted, rate-limited
virtual key so one runaway consumer cannot exhaust another's budget.

See `STATUS.md` at the repo root for what is verified against the running
LiteLLM instance.
