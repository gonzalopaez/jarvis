# JARVIS SOC v0.2 — Working Tree Baseline

Captured: 2026-09-04 (America/Argentina/Buenos_Aires)
Repository: `/home/d4rkn0d3/Projects/jarvis`

## Git identity

- Branch: `feature/qdrant-infra-rag`
- Upstream: `origin/feature/qdrant-infra-rag`
- Ahead/behind: `+0/-0`
- HEAD: `98acaa6a1115d3319ffe96cb21e794d804278828`
- Remote fetch/push: `https://github.com/gonzalopaez/jarvis.git`
- Staged files: none

## Tracked modifications present before Phase 0.5

- `Cargo.lock`
- `STATUS.md`
- `apps/desktop/src/core/state.test.ts`
- `apps/desktop/src/core/state.ts`
- `apps/desktop/src/styles.css`
- `apps/desktop/src/ui/components/security.ts`
- `apps/desktop/src/ui/template.ts`
- `apps/desktop/src/ui/view.ts`
- `docs/adr/ADR-013-qdrant-infrastructure-rag.md`
- `scripts/rag-index.py`
- `services/core/Cargo.toml`
- `services/core/README.md`
- `services/core/src/conversation.rs`
- `services/core/src/events.rs`
- `services/core/src/lib.rs`
- `services/core/src/main.rs`
- `services/core/src/routing.rs`
- `services/core/src/security.rs`
- `services/core/src/telemetry.rs`
- `services/core/src/transport.rs`
- `services/core/src/voice.rs`

## Untracked files present before Phase 0.5

- `JARVIS_SOC_V02_GAP_ANALYSIS.md` (created during approved Phase 0)
- `JARVIS_SOC_V02_IMPLEMENTATION_PLAN.md` (created during approved Phase 0)
- `PLAN.md` (pre-existing user work)
- `scripts/test_rag_index.py` (pre-existing user work)
- `services/core/src/soc_cases.rs` (pre-existing SOC implementation)

## SOC-related overlap

The highest-overlap files are `services/core/src/soc_cases.rs`, `main.rs`, `lib.rs`, `security.rs`, `events.rs`, `transport.rs`, `conversation.rs`, frontend state/UI files and `Cargo.lock`. Phase 1 must preserve these local changes and patch around them; none may be reset, stashed, cleaned or replaced wholesale.

## Warnings

- The branch contains a large uncommitted implementation (Phase 0 measured approximately 1,722 additions and 150 deletions). It is not safe to assume `HEAD` represents the running build.
- No automatic stash, reset, checkout, clean or commit was performed.
- `JARVIS_SOC_V02_PRECHANGE.diff` was not generated: a plain `git diff` would omit the three pre-existing untracked implementation files and therefore would not be a complete recovery snapshot. This manifest plus Git blob identities records state without creating a misleading partial backup. Before production work, the owner should create an approved checkpoint that includes intended untracked files.
- This document itself is the first filesystem change of Phase 0.5.

## Recovery checkpoint

After live discovery and before Phase 1 code changes, an explicit source-only archive was created at `/tmp/jarvis-pre-soc-v02-20260904.tar.gz` (134 KiB). It contains only the enumerated modified/untracked source and documentation files. It excludes `.git`, build outputs, dependency trees, `.env`, SSH material, credential files, caches and secret/token files.
