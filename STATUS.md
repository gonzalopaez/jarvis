# JARVIS status at the ADR-014 baseline

Repository baseline: `origin/main@a2f37e0` (2026-08-12).

This status separates repository evidence from production evidence. A passing
test proves the implementation at the baseline; it does not prove deployment.
Production claims below are limited to the last recorded live verification on
2026-08-11. No production `audit_id` was retained for the ADR-014 stages, so
none is asserted.

## Implemented and verified in production

| Capability | Production evidence | Repository evidence |
|---|---|---|
| Core and Codex private services | CT124 returned Core `READY` and Codex `READY` on 2026-08-11; no retained `audit_id` | Core gateway merged before `a2f37e0` |
| Voice service | CT125 service and private endpoint were active on 2026-08-11; no retained `audit_id` | Core gateway history contained by `a2f37e0` |
| MCP read gateway | CT126 was active; the Proxmox pool allow-list was verified against CT124/125 | `test_allowlist_is_the_jarvis_pool`, `test_status_rejects_vmid_outside_allowlist`; baseline `a2f37e0` |
| Prometheus host/service telemetry | Prometheus ingested 14 `jarvis_proxmox_guest_up` and 9 `jarvis_proxmox_service_up` series; the down query returned stopped guests `dc` and `freeipa` | ADR-011 and deploy artifacts contained by `a2f37e0`; no retained `audit_id` |
| LiteLLM conversation aliases | `jarvis-fast` and `jarvis-reasoning` returned responses after the recorded LiteLLM restart | LiteLLM control-plane commits contained by `a2f37e0`; no retained `audit_id` |
| n8n SOC workflow | Active `SOC 2.0` executions `2333` and `2338` completed; `ACTION_ENABLED=false` | ADR-012 artifacts contained by `a2f37e0`; production execution IDs are n8n IDs, not Core `audit_id`s |

## Implemented and validated only by tests

| Capability | Evidence | Commit/merge |
|---|---|---|
| Tier 1 immediate read authorization | `tier_1_is_allowed_immediately` | `e54124c` |
| Tier 2 single-use, session-scoped authorization | `tier_2_requires_single_use_authorization`, `grants_are_session_scoped`, `tier_3_grant_reuse_is_rejected` | `e54124c` |
| Tier 3 typed resource confirmation, rollback plan and two-minute expiry | `tier_3_without_rollback_plan_is_rejected`, `tier_3_wrong_resource_identifier_is_rejected`, `tier_3_grant_expired_at_121_seconds_is_rejected` | `e54124c` |
| Domain agents cannot self-authorize | `domain_agent_cannot_issue_its_own_grant`, `domain_agent_cannot_submit_human_confirmation` | `e54124c`, `627ad43` (PR #5) |
| Wazuh Agent bounded triage and proposal forwarding | `test_l2_triage_has_explicit_timeout_and_bounded_context`, `test_proposal_reaches_core_as_action_and_is_not_executed_by_agent` | `45aa91d`, merge `199037a` |
| Proxmox Agent proposal-only Tier 3 interface | `test_all_tier_3_capabilities_are_exposed_and_nothing_else`, `test_destroy_is_only_proposed_to_core_with_explicit_timeout` | `0c4b3dd`, merge `4131336` |
| Cross-domain parallel evidence fan-out | `cross_domain_evidence_uses_parallel_agents_route`, `cross_domain_evidence_is_requested_concurrently`, `audit_ids_remain_unique_during_concurrent_fan_out` | `5d507e2`, `e1ab502`, merge `a2f37e0` |
| Fail-closed execution boundary | `protected_actions_stop_at_authorization_boundary`, `unverified_executor_result_fails_closed`, `unknown_capabilities_are_denied_by_default` | baseline `a2f37e0` |

These tests generate in-memory `audit_id` values where applicable. They are not
production audit records and are not represented as such.

## Deployed but deliberately disabled

| Component | State | Evidence |
|---|---|---|
| `RestrictedExecutor` write path | Disabled; agents may propose actions, but no containment or infrastructure mutation is allowed to execute | Recorded production setting `ACTION_ENABLED=false`; `protected_actions_stop_at_authorization_boundary`; baseline `a2f37e0` |

## Pending or future

| Item | State at `a2f37e0` |
|---|---|
| Definitive operator identity | OIDC/WebAuthn/MFA not implemented |
| OpenBao credential broker integration | Not connected to the Core execution path |
| Capability-specific restricted write executors | Not implemented or enabled |
| Reproducible release and rollback | Not implemented |
| HUD typed Tier 3 confirmation | Not implemented |

## Trabajo pendiente de reconciliar

- `feature/voice-latency-instrumentation` is not merged. It contains the GPU
  passthrough fix and voice-latency instrumentation. Audit and merge belong to
  a separate stage.
- `feature/qdrant-infra-rag` is not merged. Its ADR-013 RAG decision has been
  reconciled with the multi-agent architecture; the obsolete, conflicting
  `ADR-014-prometheus-live-agent-context.md` direct-rendering path was dropped.
  Expanded indexing and parallel static/live evidence remain separate work.
- The `apps/desktop` Agent Matrix still exposes the older categories `VOICE
  ENGINE`, `N8N`, `SECURITY AGENT` and `MCP GATEWAY`; it does not represent the
  Wazuh Agent and Proxmox Agent roster. HUD work belongs to a separate stage.
- Production access used by Claude Code is an unrestricted Proxmox root SSH
  key, not a technically read-only credential. Define graduated credentials:
  a read-only Proxmox API token for diagnostics and root SSH reserved for
  authorized write operations.
