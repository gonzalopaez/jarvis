# SOC Schema v0.2 Proposal

Status: **reconciled with verified CT133 schema; local SQL prepared, not applied**
Evidence: `docs/discovery/JARVIS_SOC_CT133_SCHEMA_2026-09.sql`.

## Design rules

- Extend the existing `soc_cases` and evidence/event structures; never replace them.
- Case row stores current operational projection. Immutable/history tables store assessments, analyst verdict changes and activity.
- JSONB is appropriate for versioned factor/evidence snapshots, but searchable identities, verdicts, timestamps and scores remain typed columns.
- AI and analyst verdicts are separate domains and histories.
- All timestamps use `timestamptz`; scores use constrained small integers; versions are nonempty text.
- Preserve current `priority` until every reader uses `initial_priority`/`final_priority`.

## Conditional object model

Names are reconciled against the three live public tables. `soc_assessments`, `soc_feedback` and v0.2 case columns do not currently exist.

### Existing `soc_cases` — additive current projection

Candidate nullable columns:

| COLUMN | TYPE | PURPOSE |
|---|---|---|
| `risk_score` | smallint check 0..100 | Latest accepted risk |
| `risk_level` | text/check or existing enum | LOW..CRITICAL |
| `ai_confidence` | smallint check 0..100 | Latest accepted confidence |
| `ai_verdict` | text/check or enum | Latest AI verdict projection |
| `analyst_verdict` | text/check or enum | Latest analyst verdict, default/semantic PENDING |
| `analyst_reason` | text | Latest reason code/text |
| `analyst_notes` | text | Latest bounded notes |
| `initial_priority` | compatible type with current priority | Preserved Wazuh/asset priority |
| `final_priority` | compatible type | Post-assessment priority |
| `priority_reason` | text | Explain current final priority |
| `assigned_at` | timestamptz | Assignment time |
| `acknowledged_at` | timestamptz | ACK time |
| `investigation_started_at` | timestamptz | Investigation start |
| `resolved_at` | timestamptz | Resolution |
| `closed_at` | timestamptz | Closure |
| `ack_sla_deadline` | timestamptz | Backend-computed ACK deadline |
| `investigation_sla_deadline` | timestamptz | Backend-computed investigation deadline |
| `sla_status` | text/check or enum | ON_TIME/WARNING/BREACHED |
| `assessment_version` | text | Latest contract version |
| `scoring_version` | text | Latest risk rules version |

Do not add fields already represented equivalently in live schema. Analyst identity/assignment columns must reuse existing principal representation.

### `soc_assessments` — append-only assessment history

Candidate fields:

- `assessment_id` generated primary key (type consistent with live conventions)
- `case_id` FK to existing case PK
- `created_at`, `completed_at`
- `model_alias`, `analysis_level` (`L1`/`L2`)
- `assessment_version`, `scoring_version`, `confidence_version`
- `ai_verdict`, `confidence_score`, `risk_score`, `risk_level`
- `summary`, `hypothesis`
- `risk_factors` JSONB array of `{factor,value,points,reason}`
- `positive_points`, `negative_points`
- `supporting_evidence`, `contradicting_evidence`, `missing_information` JSONB arrays of references
- `mitre_correlation` JSONB, `recommendations` JSONB
- `evidence_package_version`, `evidence_snapshot` JSONB or reference to normalized evidence package
- `supersedes_assessment_id` nullable self-FK when relevant

L1 and L2 are independent rows. Neither overwrites the other. The case projection changes only after a validated assessment is accepted transactionally.

### Evidence / MITRE

First inspect current `case_events.evidence`. Preferred choices:

1. If existing event/evidence rows already have stable IDs, add canonical normalized payload/version and reference those IDs from assessments.
2. Otherwise add `soc_case_evidence` with `evidence_id`, `case_id`, source/source_id/timestamp/raw_reference/normalized_fields/schema_version and a uniqueness rule appropriate to source+source_id.

Structured MITRE may remain in canonical evidence JSONB initially, with a child `soc_case_mitre` table only if filtering/reporting/query plans justify it. Do not duplicate MITRE into Qdrant as source of truth.

### Analyst verdict history / feedback

Candidate append-only `soc_feedback`:

- feedback ID, case ID, optional alert/rule/assessment IDs
- prior and new analyst verdict
- reason, notes, analyst principal, timestamp, request ID
- snapshot of AI verdict/confidence/risk used for agreement calculation

Current `soc_cases.analyst_verdict` is only a projection. Every change inserts feedback/activity; AI verdict is immutable from this operation.

### Activity/audit

Reuse a live audit/activity table if one exists. Otherwise add append-only SOC activity with timestamp, principal, case, request ID, event/action, before/after, source and success. Do not store secrets or unbounded raw logs.

## SLA model

- Deadlines are materialized when initial/final priority becomes authoritative, using backend configuration and database timestamps.
- Current projection stores deadlines/status; activity records changes and breaches.
- Priority changes do not silently erase original deadlines. Recalculation policy must be explicit and audited.
- Derived MTTA = acknowledged_at - created_at; time-to-investigation = investigation_started_at - created_at; MTTR definition must select resolved_at or closed_at consistently.
- Scheduler implementation is outside this phase, but indexes should eventually support open cases ordered by deadline.

## Verdict domains

AI: `FALSE_POSITIVE`, `BENIGN_POSITIVE`, `SUSPICIOUS`, `TRUE_POSITIVE`, `INCONCLUSIVE`.
Analyst: same plus `PENDING`.

Prefer check constraints initially if adding enum values later may be operationally cumbersome. Reuse compatible live enums if present.

## Risk Engine v1 design

Single versioned configuration/module; deterministic saturating result 0..100. Suggested initial factors are a design requiring calibration against historical cases:

| FACTOR | INPUT | POINT MODEL |
|---|---|---|
| Wazuh level | integer/null | monotonic bounded base contribution |
| Asset criticality | known classification | additive impact points |
| Privileged identity | explicit inventory/evidence | additive; unknown gives 0, not false |
| Correlated alerts | unique stable alert IDs | capped nonlinear points |
| MITRE techniques/tactics | Wazuh-provided only | capped diversity points |
| Temporal progression | deterministic relationship engine | points only for ordered related evidence |
| IOC | validated evidence/reference | points by quality, capped |
| Historical TP | comparable prior cases | bounded positive adjustment |
| Historical benign | comparable prior cases | bounded negative adjustment |
| Recurrence | defined time window | capped adjustment |

Output: score, level, positive/negative totals, version, calculated time and every factor `{factor, raw_value, points, reason}`. Missing values contribute zero and an explanation. The LLM never supplies points.

Risk levels: 0–29 LOW, 30–49 MEDIUM, 50–69 HIGH, 70–89 VERY_HIGH, 90–100 CRITICAL.

## Confidence Engine v1 design

Confidence measures support for the verdict, not impact. It consumes validated evidence references and normalized LLM components:

- deterministic base by source quality;
- capped unique evidence count;
- temporal correlation strength;
- independent-source correlation (not duplicate adapters of the same Wazuh event);
- validated historical similarity;
- explicit deductions for contradictions;
- explicit deductions for missing evidence relevant to the hypothesis.

Output includes score 0..100, version and component explanations. Duplicate evidence contributes once. An unavailable source is missing information, not contradictory evidence. Contradictory/missing references from LLM are accepted only if they resolve to the Evidence Package.

Exact weights must be fixed in one reviewed configuration after historical calibration and unit fixtures; they are intentionally not invented before live schema/data-shape verification.

## Required indexes (conditional)

- Assessments by `(case_id, created_at desc)`.
- Open case work queue using existing status/final priority/SLA columns.
- Evidence source identity unique/indexed only where stable IDs exist.
- Feedback by `(case_id, created_at)` and low-cardinality aggregate dimensions.

Use `CREATE INDEX CONCURRENTLY` separately for large populated tables after measuring size; it cannot run inside a transaction block. No index DDL is finalized yet.
