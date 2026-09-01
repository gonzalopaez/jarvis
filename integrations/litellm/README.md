# LiteLLM integration

LiteLLM is the model control plane (ADR-002): Jarvis Core and the n8n SOC
workflow are its clients, and physical model choices live in `config.yaml`, not
in caller code.

## What is active

- **Conversation routing** — `services/core/src/voice.rs` calls LiteLLM through
  named aliases rather than physical model names. Both `jarvis-fast` and
  `jarvis-reasoning` are live and allowed by Core's scoped key/team policy.
- **SOC triage aliases** — `jarvis-soc-l1` and `jarvis-soc-l2` (ADR-012) are the
  aliases consumed by the active n8n "SOC 2.0" workflow for L1/L2 alert triage.
  They require structured output against
  `contracts/api/security-verdict.v1.schema.json`, replacing the workflow's
  previous free-text regex parsing.

## What remains future

- MCP and Agent Gateway routing *through* LiteLLM (ADR-008) is not wired yet.
- Executing approved actions remains future. Wazuh Agent already forwards
  allow-listed `proposed_actions` to Core as action requests; Core stops them at
  the authorization boundary while `RestrictedExecutor` is disabled. Evidence:
  `test_proposal_reaches_core_as_action_and_is_not_executed_by_agent`,
  `protected_actions_stop_at_authorization_boundary`, merge `199037a`.

## config.yaml

`config.yaml` is a **sanitized desired-state reference**, not a complete copy of
the live server configuration. It must be merged with the deployed model list;
replacing the live file verbatim would remove unrelated aliases. Master keys,
database URLs and consumer keys are never committed.

n8n uses separate budgeted, rate-limited keys for chat triage
(`n8n-soc-triage`, restricted to `jarvis-soc-l1/l2`) and embeddings
(`n8n-soc-embeddings`, restricted to `nomic-embed-text`). This preserves least
privilege while allowing the workflow's RAG pre-filter to operate.

See `STATUS.md` at the repo root for what is verified against the running
LiteLLM instance.
