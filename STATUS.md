# JARVIS deployment status

Last verified against the live server: **2026-08-11**.

Records what was tested directly against running services, not what the code is
expected to do. The deployed `RestrictedExecutor` remains intentionally disabled.

> Note: a broader deployment-status section (HUD, Prometheus exporter, MCP test)
> lives on branch `feature/deploy-gap-closure` and will merge into this file.

## LiteLLM SOC control plane (ADR-012)

Verified on the real LiteLLM instance (192.168.1.11:4000, CT116, Ollama-backed):

- **Aliases live.** `/v1/models` lists `jarvis-soc-l1` (→ ollama/llama3.2) and
  `jarvis-soc-l2` (→ ollama/qwen2.5) alongside the pre-existing models. The merge
  was additive: the 7 existing models (incl. `jarvis-general`, `jarvis-fast`,
  `nomic-embed-text`) were preserved. Config backed up before the change.
- **Structured output works on the local models.** Both `jarvis-soc-l1` and
  `jarvis-soc-l2` returned schema-valid JSON (`verdict` / `confidence` /
  `justification`) when called with `response_format: json_schema`. L1 returned
  `FALSO_POSITIVO`, L2 returned `AMENAZA_REAL_BAJA` with a full justification on
  the same test alert. No PARSE_ERROR path was needed in this manual test — but
  n8n must still validate every response against the schema (see ADR-012).
- **Scoped virtual key issued.** `n8n-soc-triage`: models restricted to
  `jarvis-soc-l1`/`l2`, `max_budget` 5 / 24h, `rpm_limit` 60. Verified it is
  rejected (401) when calling an out-of-scope model. Key stored root-only at
  `/etc/litellm/n8n-soc-triage.key` on CT116; NOT in git. Destined for the n8n
  credential store (or OpenBao).
- **Core unaffected.** No change to `services/core/src`; after the LiteLLM
  restart, Core and Codex report `READY` and `jarvis-fast` still responds.

### Not yet done / known gaps

- **n8n workflow reimport is pending**: the patched workflow file
  (`SOC_2_0_patched.json`) was not available, so the workflow was not touched.
- **`jarvis-reasoning` is not defined** on the live LiteLLM, although
  `services/core/src/voice.rs` references it. Pre-existing gap; left as-is to
  avoid touching Core routing in this task.
- Forwarding a verdict's `proposed_actions` to Core (the n8n↔Core action
  contract) is **not implemented**. Until `PolicyEngine` has rules for
  `security.user.disable` / `security.ip.block` / `security.host.isolate`, any
  forwarded action would be denied `CAPABILITY_DENIED`, which is the correct
  default.
