# ADR-012: LiteLLM as the real control plane for SOC triage (L1/L2)

## Status

Proposed — 2026-08-11.

## Context

ADR-002 already decided that model routing lives behind LiteLLM, with Jarvis
Core as its client. In practice today:

- `services/core/src/voice.rs` calls LiteLLM correctly, through named virtual
  models (`jarvis-fast`, `jarvis-reasoning`), matching `docs/ai-routing.md`.
- The SOC triage workflow (n8n, `SOC 2.0`) also calls the same LiteLLM
  instance (`192.168.1.11:4000`), but with **physical model names hardcoded**
  in the workflow JSON (`"model": "llama3.2"`, `"model": "qwen2.5"`), and
  parses the model's free-text response with regex
  (`Parse L2 + Extract Actions`).
- `integrations/litellm/README.md` still says "Future... No integration is
  active", which is stale: the integration is live for voice/conversation,
  just not for SOC triage, and not committed as config anywhere in the repo.

This means the system has two LiteLLM consumers with very different levels of
control, and the one that can eventually trigger real actions (L2 verdict →
block user / block IP / isolate host) is the less-controlled one.

## Decision

1. **Add two SOC-specific virtual model aliases in LiteLLM**, mirroring the
   existing `jarvis-fast` / `jarvis-reasoning` pattern:
   - `jarvis-soc-l1` → fast local model, low temperature, triage only.
   - `jarvis-soc-l2` → stronger local model, lowest temperature, deep IR
     analysis.

   n8n calls these aliases, never a physical model name. Swapping the
   underlying model is a LiteLLM config change, not a workflow edit.

2. **Require structured output** (`response_format: json_schema`) on both
   aliases, against `contracts/api/security-verdict.v1.schema.json`. This
   removes the regex parsing step in n8n entirely and removes the main
   source of fragility flagged earlier: a malformed free-text response can no
   longer silently produce a wrong or missing action flag.

3. **Separate virtual keys per consumer**, each with its own budget and rate
   limit, issued via LiteLLM's key-management API (never committed to git —
   same rule as every other credential in this repo):
   - `core-conversation` (existing Core usage)
   - `n8n-soc-triage` (new)

   This gives per-consumer audit trail and stops a runaway triage loop in n8n
   from exhausting the budget used by real-time conversation.

4. `proposed_actions` inside the verdict schema reuses the **same**
   `capability` / `target` vocabulary as
   `contracts/api/core-request.v1.schema.json#/$defs/action`. A verdict is
   never auto-executed by n8n; it is forwarded as a normal Core action
   request, evaluated by the same `PolicyEngine` and (currently disabled)
   `RestrictedExecutor` as any other action in the system. No new execution
   path is created by this ADR.

## Consequences

- `integrations/litellm/README.md` must be updated to reflect what's real:
  conversation routing is live, SOC aliases are new, MCP/Agent Gateway
  routing through LiteLLM remains future.
- `PolicyEngine` still needs real rules for `security.user.disable`,
  `security.ip.block`, `security.host.isolate` (tracked separately —
  STATUS.md gap #4). Until those rules exist, any forwarded action is denied
  with `CAPABILITY_DENIED`, which is the correct default.
- Local models (Ollama-backed) have weaker native structured-output support
  than hosted providers. LiteLLM enforces what it can at the proxy layer;
  n8n's parser should still validate the response against the schema and
  route to a `FALSO_POSITIVO`-equivalent "needs human review" path on
  validation failure, rather than trusting the model unconditionally.

## Alternatives considered

- **Leave n8n calling physical model names directly.** Rejected: keeps two
  divergent LiteLLM usage patterns in the same system and keeps the regex
  parser as the only thing standing between a model's free text and a
  proposed real-world action.
- **Move triage logic into Core instead of LiteLLM aliases.** Deferred: this
  is the longer-term direction (see the Core-as-orchestrator discussion), but
  requires Core to have its own correlation engine first. Aliasing at the
  LiteLLM layer is the change with the best effort/value ratio today.
