# ADR-014: Multi-agent architecture with centralized authorization

## Status

Proposed — 2026-08-11. Supersedes the implicit split between Core, n8n and MCP
Gateway that grew organically. Does not require a rewrite: existing components
are re-scoped, not discarded.

## Context

The system currently has reasoning happening in three uncoordinated places:
Core's `ConversationService`/`routing.rs`, n8n's L1/L2 triage (Qdrant + Ollama
via LiteLLM), and — as of `feature/qdrant-infra-rag` — a second RAG path
inside Core itself. Each grew independently because it was the path of least
resistance at the time, not because it was the right owner for that logic.
`integrations/n8n/README.md` already states the intended constraint
("sanitized workflow templates only... internal addresses are prohibited")
that `SOC 2.0` violates in practice.

Separately, the user wants a real multi-agent system: Jarvis Central talking
to a Wazuh agent (alerts, telemetry, containment actions) and a Proxmox agent
(infrastructure state **and deployment**). Multi-agent does not, by itself,
fix the multi-brain problem — it can make it worse if each agent reasons
independently. It also introduces a capability category the system has never
had: creating/destroying infrastructure, which carries materially higher
blast radius than the containment actions (`security.user.disable`,
`security.ip.block`, `security.host.isolate`) designed so far.

## Decision

### 1. One reasoning brain, distributed evidence

Jarvis Central owns the only LLM used for judgment (Codex, via the
`jarvis-fast` / `jarvis-reasoning` LiteLLM aliases, extended to absorb the
`jarvis-soc-l1` / `jarvis-soc-l2` triage aliases from ADR-012). Domain agents
never form verdicts. An agent's only job is: expose MCP tools (read + a
declared set of *proposable* actions) and translate its system's raw state
into evidence Central's reasoning can consume. This directly resolves the
"multiple brains" issue diagnosed earlier in this project.

n8n is demoted back to what its own README already says it should be:
mechanical correlation (grouping alerts by host/user/time window) and
notification plumbing feeding the Wazuh agent — never verdict formation,
never direct calls to FreeIPA/Wazuh Active Response.

### 2. Capability tiers, not a single authorization rule

Every capability in the (still-to-be-populated) `PolicyEngine` gets a tier.
Tier is a first-class field of the capability, not an ad hoc branch in code:

| Tier | Examples | Authorization |
|---|---|---|
| 1 — read-only | `wazuh.alerts.read`, `proxmox.guest.status`, `core.health.read` | None. Any agent, any time. |
| 2 — reversible containment | `security.user.disable`, `security.ip.block`, `security.host.isolate` | One human, single-use grant, 5-minute expiry (existing `ConversationService` confirmation pattern). |
| 3 — infrastructure create/destroy | `proxmox.vm.deploy`, `proxmox.vm.destroy`, `proxmox.ct.destroy` | One human, **typed confirmation of the exact resource name/ID**, 2-minute expiry, mandatory `rollback_plan` field populated before the confirmation UI is even shown. |

Tier 3 is new — nothing in the repo has this shape today. It is deliberately
more expensive to trigger than Tier 2: destroying infrastructure is not
symmetric with containing a threat, and the authorization UX should feel
different, not just gate the same button behind one more click.

### 3. Agent roster and what each one owns

- **Jarvis Central** — orchestrator, `PolicyEngine` (with the tiers above),
  the reasoning LLM, and the shared `RestrictedExecutor` (still disabled,
  unaffected by this ADR).
- **Wazuh agent** — evidence: alerts, agent telemetry, FIM events.
  Proposable capabilities: Tier 2 containment actions only.
- **Proxmox agent** — evidence: guest status, resource usage.
  Proposable capabilities: Tier 1 reads (existing MCP Gateway scope) **plus**
  new Tier 3 deploy/destroy capabilities.
- **Future agents** (LiteLLM cost/routing, OpenBao credential lifecycle,
  whatever comes next) follow the identical pattern: MCP tools + evidence
  adapter, zero reasoning of their own, capabilities registered in the same
  tiered table.

Codex CLI (OpenAI's actual CLI agent, with real shell/filesystem access) was
explicitly evaluated and **excluded from this architecture**. It doesn't fit
the propose-then-authorize model because its execution and its proposal are
the same event — by the time a human reviews it, the action already happened.
If it's adopted later, it stays outside the agent-authorization loop entirely
(sandboxed branch, human-reviewed PR as the actual gate), not as a peer of
the Wazuh/Proxmox agents.

## Consequences

- `PolicyEngine::default()` needs real rules with tiers, not the 2 placeholder
  rules it has today (tracked as STATUS.md gap #4 — this ADR defines the
  shape those rules should take).
- The n8n↔Core action-forwarding contract discussed earlier (verdict →
  `kind: "action"` request) still applies, but the verdict itself now
  originates from Central's reasoning over evidence the Wazuh agent supplied
  — not from n8n's own L1/L2 nodes deciding independently.
- A new capability category (Tier 3) needs its own confirmation UI in the
  HUD — the existing 5-minute/one-time confirmation component isn't
  sufficient as-is; it needs a typed-confirmation variant.
- `feature/qdrant-infra-rag` should be re-evaluated against this ADR before
  merging: is it evidence retrieval for Central's reasoning (fits), or is it
  quietly becoming a second reasoning path (doesn't fit)?

## Alternatives considered

- **Each agent with its own LLM** (option 1 from the earlier discussion).
  Rejected: reintroduces the multi-brain problem this ADR exists to solve,
  just distributed across more services instead of two.
- **Single authorization tier for all actions.** Rejected: treats destroying
  infrastructure the same as blocking an IP, which understates the former's
  blast radius and doesn't match how a human SOC analyst would actually want
  to be asked.
