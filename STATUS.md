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
  `/etc/litellm/n8n-soc-triage.key` on CT116 and encrypted in n8n's credential
  store; NOT in git.
- **Embeddings key repaired and isolated.** The inherited generic bearer token
  was invalid. `n8n-soc-embeddings` is restricted to `nomic-embed-text`, with
  `max_budget` 2 / 24h and `rpm_limit` 60, and is attached only to the RAG
  embeddings node.
- **n8n workflow active and tested.** `SOC 2.0` was backed up before import,
  patched in place (workflow ID preserved), published and tested with synthetic
  L1 and L2 alerts. L1 execution `2333` completed with schema-valid JSON. L2
  execution `2338` completed end to end through Wazuh enrichment, LiteLLM,
  `Parse L2 + Extract Actions`, Telegram branches and Qdrant persistence.
  The L2 parser reported `needsReview=false`, no `PARSE_ERROR`, populated
  `veredicto` / `confianza` / `actions.proposed`, and kept
  `ACTION_ENABLED=false`.
- **Pre-existing workflow defects repaired.** The stale LiteLLM embeddings
  token, missing Wazuh API credential reference and incorrect Qdrant
  `POST /points` method were replaced with scoped credentials and the correct
  `PUT` upsert. Credentials remain only in n8n's encrypted store.
- **Core remained healthy.** After the LiteLLM restart, Core and Codex report
  `READY` and `jarvis-fast` still responds.
- **Reasoning route repaired.** `jarvis-reasoning` is now deployed against
  `ollama/qwen2.5`; both the `jarvis-core-voice` key and `jarvis-core` team are
  restricted to `jarvis-fast` / `jarvis-reasoning`. A completion through the
  new alias was verified from CT124 with Core's real credential.

### Not yet done / known gaps

- Forwarding a verdict's `proposed_actions` to Core (the n8n↔Core action
  contract) is **not implemented**. Until `PolicyEngine` has rules for
  `security.user.disable` / `security.ip.block` / `security.host.isolate`, any
  forwarded action would be denied `CAPABILITY_DENIED`, which is the correct
  default.
