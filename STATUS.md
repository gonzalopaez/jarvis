# JARVIS status at the ADR-014 baseline

Repository baseline: `origin/main@e342d54` (2026-09-11).

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
| Phase 1.5 durable Tier 1 skill outbox (`wazuh.alerts.read`) | Live `POST /api/v1/tier1/wazuh/alerts` against the real Wazuh relay returned `status: verified`, `audit_id: audit-3050cb0cce602a239ad4bfe51ad3dab3`, `event_id: event-7e4d57caae8010bb1773a5fec09fb029`; the outbox row reached `state: delivered` on one attempt; the resulting point (`25e60c5d-6533-4a53-7294-1f97eddac70f`) was upserted into the real `jarvis_skill_memory_v1` Qdrant collection (CT115) and a subsequent same-`task_type` query with a realistic prompt retrieved it at `score: 0.663` (above the 0.60 threshold), with the correct filtered payload | PR #8 `4daafb3` (outbox/migration/adapter) + PR #9 `e342d54` (LiteLLM https scheme fix, required for this to start); dedicated Core PostgreSQL provisioned on new CT136 (`jarvis-core-db`, 192.168.1.30), migrated with `scripts/core-migrate.sh` |

## Production incidents

### Phase 1.5 rollout cascade — resolved 2026-09-11

Deploying PR #8/#9 surfaced five separate, previously-latent production gaps,
each blocking the next. None were introduced by this rollout; the rollout was
simply the first thing to actually exercise these paths end-to-end.

| # | Gap found | Resolution |
|---|---|---|
| 1 | CT124/125/126 (Core, Voice, MCP) were stopped, contradicting the last recorded live state | Started; no data loss, systemd `active` on all three |
| 2 | `VoicePipeline::new`/`KnowledgeClient::new`/`SkillMemoryClient::new` rejected any `litellm_base_url` scheme other than `http`; production reaches LiteLLM only via `https://codex-llm.d4rkn0d3.com/`, so Core entered a startup crash-loop the moment the Phase 1.5 env vars were wired in | Fixed in reviewed PR #9 (`e342d54`); endpoints that are genuinely `http`-only (voice service, Qdrant, Wazuh relay, Codex, Prometheus) were left untouched |
| 3 | The `jarvis_core_migrator`/`jarvis_core` roles and `jarvis_core` database did not exist yet on CT136; the app role's default privileges were also granted on schema `public`, not on schema `jarvis_core`, where the outbox tables actually live | Roles, database and firewall (nftables, CT124-only on 5432) provisioned on new CT136; corrected `GRANT`/`ALTER DEFAULT PRIVILEGES` scoped to schema `jarvis_core` |
| 4 | CT135 (`litellm-codex`), the LiteLLM instance behind `codex-llm.d4rkn0d3.com`, was stopped; once started, its DB-backed virtual keys (`rag-embeddings-token`, `skill-memory-embeddings-token`) failed every call with `"No connected db."` — this LiteLLM instance has no `database_url` configured and only its master key bypasses that check | Started CT135; `skill-memory-embeddings-token` on CT124 repointed to the same value as `litellm-token` (the working master key). `rag-embeddings-token` was left as-is — RAG remains unconfigured/unvalidated, out of this rollout's scope |
| 5 | CT135's `config.yaml` had no `jarvis-embed-multilingual` model entry at all (only the three `codex-hermes-8b` chat models); embeddings had nowhere to route even with working auth | Added a `jarvis-embed-multilingual` → `ollama/bge-m3` entry proxying to CT116's Ollama backend (192.168.1.11:11434), matching CT116's own working definition; the three existing chat model entries were left untouched |

A destructive-operation mistake also occurred during CT136 provisioning: a
malformed `pct create` (unquoted `--tags` value split by the shell) collided
with pre-existing VMID 128 (QEMU VM "Tails", unrelated). Proxmox refused the
create, but the subsequent cleanup deleted that VM's pre-existing `unused0`
disk volume without checking its content-type first — it was assumed to be
debris from the failed command. The user confirmed no data of consequence was
lost. A standing verification rule (VMID match, content-type, ownership scope)
was recorded before any further destructive storage operation this session.

### Prometheus CT127 disk exhaustion — resolved 2026-09-01

| Field | Evidence |
|---|---|
| Failure began | Prometheus journal first recorded `no space left on device` during TSDB head compaction on 2026-08-30 at 20:00:04 -03 |
| Detection | Detected on 2026-09-01 through the Server Central HUD remaining at `--`; CT127 had entered a restart loop with more than 1,500 attempts |
| Root cause | The 4 GB root filesystem had no compaction margin. The configured `7d` / `2GB` block retention does not include the WAL, head chunks, operating system or temporary compaction space; the uncompactable WAL reached 2.73 GB |
| Backup | The stopped TSDB was archived before recovery as `prometheus-tsdb-20260901T212640-0300.tar.zst`; SHA256 `e86dabb817dbeaa2c31fef5d8d069826bdebb8c5afb3766bd36f95ccbc62bde2` |
| Resolution | CT127 rootfs was expanded from 4 GB to 16 GB at 2026-09-01 21:27 -03. No TSDB or WAL data was deleted. Prometheus returned healthy with zero restarts and all 13 Server Central queries returned one valid result from CT124 |
| Prevention | Preserve the explicit `--storage.tsdb.retention.size=2GB` limit with sufficient filesystem headroom and alert when CT127 root free space remains below 15% for 10 minutes |

Visual confirmation of the recovered values in the operator's actual HUD is
still required; HTTP, PromQL and Core-to-Prometheus verification do not replace
that check.

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
| Router-authoritative text/voice, RAG and model fallback | `router_alias_is_preserved_without_qdrant_context`, `qdrant_context_does_not_override_router_alias`, `router_owns_codex_fallback_and_cross_domain_model_decisions`, `conversation_and_transport_do_not_fix_model_aliases` | `0184eaf`, merge `17049f0`; not deployed |
| Read-only governed skill memory | `skill_memory_enriches_context_without_overriding_router_alias`, `renders_only_bounded_active_matching_skills`, `documentary_collection_cannot_be_reused` | `26a14be`, merge `17049f0`; not deployed |
| Verified-outcome skill write boundary | `verified_outcome_requires_durable_execution_and_tier_authorization`, `point_identifier_is_stable_and_qdrant_compatible` | `af6b39f`, merge `17049f0`; boundary only, no runtime producer, not deployed |
| Phase 1.5 Tier 1 durable skill pipeline | `real_wazuh_read_path_validates_and_returns_only_typed_counts`, `tier_one_policy_accepts_only_curated_wazuh_records`, `tier_two_and_three_are_explicitly_disabled_before_writing`, `tier_one_write_is_idempotent_and_retrievable` | merge `4daafb3` (PR #8), `e342d54` (PR #9); deployed and validated in production 2026-09-11, see "Implemented and verified in production" |
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
| RAG (`jarvis_knowledge_bge_v1`) production activation | Env vars now wired (required as a side effect of enabling skill memory — `JARVIS_QDRANT_URL` is shared), but `rag-embeddings-token` is a DB-backed LiteLLM virtual key that CT135 cannot validate (`"No connected db."`). Never exercised in production; not fixed in this rollout |
| Agent Matrix live status for Voice/MCP/n8n/Wazuh | `/api/v1/health` reports all four as `OFFLINE`/`not_connected` even when the underlying services answer their `v1/health`/`healthz` routes directly and the Wazuh Tier 1 pipeline is independently proven working end-to-end. The `AgentHealthPoller`/event consumption path needs its own investigation; not addressed in this rollout |
| CT135 (`litellm-codex`) database | No `database_url` configured; only the master key bypasses its `user_api_key_auth` DB check. Any caller using a DB-backed virtual key (not the master key) will fail. Whether CT135 is meant to have its own DB restored, or all callers are meant to move to the master key, is an open decision |

## Trabajo pendiente de reconciliar

- The AMD Radeon RX 5600 XT at PCI `0000:03:00` is shared by configuration:
  VM110 (`Ubuntu-RDP`) declares it as `hostpci0`, while CT116
  (`originalOllama`) consumes the host `amdgpu` DRM devices. They cannot use
  the GPU simultaneously. CT116 is the default consumer as of 2026-09-01; do
  not start VM110 without stopping CT116 first. A future scheduling or
  ownership policy must make this exclusion enforceable instead of relying on
  operator procedure.
- CT116 GPU inference was verified on 2026-09-01 at 165 generated tokens/s
  with all 17 model layers loaded through Vulkan. This verifies the current
  running host/container state, not boot persistence. Survival of a complete
  Proxmox host reboot remains unverified and requires a coordinated downtime
  window.
- The remaining commits on `feature/voice-latency-instrumentation` are not
  merged as a unit. Their unrelated scope must continue through separate audit
  and merge stages; the historical GPU commit on that branch is superseded by
  the reconciled v2 hook and configuration in this branch.
- `feature/qdrant-infra-rag` and the governed skill-memory stages were merged
  through PR #6 as `17049f0`. Qdrant reindexing, creation of
  `jarvis_skill_memory_v1`, delivery of its credential and Core deployment
  remain pending; no production activation is inferred from the merge.
- The `apps/desktop` Agent Matrix now shows the real roster (Voice Service,
  MCP Gateway, n8n, Wazuh Agent, Proxmox Agent) with live polling for
  Voice/MCP/n8n and a permanent `NOT INSTRUMENTED` state for Proxmox Agent
  (`feature/hud-real-agent-roster`, merged 2026-09-02). This closes repository
  and test evidence; the corresponding production build/deploy to Nginx and
  operator visual confirmation are still pending as of this merge.
- Production access used by Claude Code is an unrestricted Proxmox root SSH
  key, not a technically read-only credential. Define graduated credentials:
  a read-only Proxmox API token for diagnostics and root SSH reserved for
  authorized write operations.
- New infrastructure from the Phase 1.5 rollout (2026-09-11): CT136
  (`jarvis-core-db`, 192.168.1.30) is a dedicated, unprivileged PostgreSQL LXC
  for the `jarvis_core` audit/outbox schema only, `onboot: 0` like CT133,
  nftables-restricted to CT124 on 5432. Its `jarvis_core_migrator` and
  `jarvis_core` role credentials live in OpenBao at
  `jarvis-platform/core/postgres` (host, port, database, migrator/app user
  and password fields) — same pattern referenced for `trading_gpu`, on a
  separate KV mount (`jarvis-platform/`, not `secret/`). Migration privileges
  (`jarvis_core_migrator`) are intentionally separate from the runtime app
  credential (`jarvis_core`, `core-database-password` systemd credential on
  CT124); Core never migrates its own schema at startup.
