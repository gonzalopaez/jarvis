# ADR-015: Skill memory as bounded, non-authoritative evidence

## Status

Accepted for staged implementation — 2026-09-11.

Read-only retrieval may be enabled independently. Automatic writes remain
disabled until Core can consume the durable success signal defined below.

## Context

ADR-013 gives Core bounded documentary evidence from Qdrant. ADR-014 makes
Core's `CapabilityRouter` the sole owner of model selection and keeps domain
agents outside the reasoning and authorization boundaries. Reusable experience
must preserve both decisions: a previous approach can inform a new task, but it
cannot select a model, grant a capability, authorize an action, or constitute
proof that an action succeeded.

The current event bus is process-local. `authorization.approved` proves only
that a human issued a short-lived grant, while an `AuditEvent` with outcome
`verified` is not durably stored by every runtime configuration. Neither alone
is a sufficient learning signal.

## Decision

### Storage and isolation

Experience is stored as Qdrant points in the dedicated collection
`jarvis_skill_memory_v1`. It must never share the ADR-013 collection
`jarvis_knowledge_bge_v1`. Separate collections permit independent credentials,
retention, payload indexes, backups and deletion without mixing reviewed
documentation with learned experience.

The point vector is the embedding of a bounded retrieval document composed from
`task_type`, `context_summary` and `approach`. The vector is Qdrant point data;
it is not duplicated in the payload.

Payload contract `jarvis.skill.v1`:

```json
{
  "schema_version": "jarvis.skill.v1",
  "skill_id": "stable opaque identifier",
  "task_type": "bounded normalized category",
  "context_summary": "bounded redacted summary",
  "approach": "bounded reusable principle, not instance instructions",
  "outcome": "success",
  "capability": "capability catalog identifier",
  "capability_tier": 1,
  "human_confirmed": false,
  "source_event_id": "task_outcome event identifier",
  "source_audit_id": "Core audit identifier",
  "created_at": "RFC 3339 timestamp",
  "expires_at_epoch_seconds": 1800000000,
  "revoked": false
}
```

`skill_id`, `source_event_id` and `source_audit_id` are provenance, not model
input. The writer uses a deterministic point identifier derived from the source
event so retries are idempotent. Payload validation rejects unknown fields,
control characters, secret-shaped data, unsupported tiers and overlong values.

### Retrieval

Core embeds the current task through LiteLLM and searches
`jarvis_skill_memory_v1` with a `task_type` filter. It requests only active,
unexpired `outcome=success` points. At most three results and 8 KiB of rendered
skill context are accepted within the existing eight-second retrieval budget.

Rendered skills are explicitly marked untrusted historical evidence. They are
injected only into the model context after `CapabilityRouter` has produced its
typed model decision. Retrieval cannot alter `ModelDecision`, capability tier,
authorization, tool availability or executor behavior. Failure or malformed
results degrade to no skill context.

### Durable write signal

The sole automatic write input is `task_outcome.verified.v1`, durably committed
by Core after all of these facts are known:

1. the capability exists in the catalog and its tier is recorded;
2. the restricted executor returned `verified=true` for the exact request;
3. the reusable approach and context summary passed bounds and redaction;
4. Tier 1 records identify the authenticated initiating subject; and
5. Tier 2/3 records reference a durable human authorization for the same
   request, subject, capability and target.

The event and its referenced authorization must commit before Qdrant is called.
The writer consumes committed events idempotently and records delivery status;
Qdrant availability never changes the action outcome. Process-local events,
model self-reports, successful HTTP status, proposals, grants without verified
execution, and analyst verdicts without a matching executed capability are not
valid learning signals.

The current disabled executor cannot produce this signal. Therefore this stage
implements the validated writer boundary but does not connect it to ordinary
conversation completion or enable production writes.

### Retention and revocation

Skills expire 180 days after creation by default. Epoch seconds are used so
Qdrant can apply an exact numeric range filter. Retrieval excludes expired or
revoked points even before physical deletion. Operators may revoke by
`skill_id`, `source_event_id` or compromised audit range. A maintenance job may
delete expired points after the relevant PostgreSQL audit/outbox retention and
backup requirements are satisfied.

Raw prompts, model transcripts, credentials, tokens, IP authentication data and
exact destructive commands are not stored. `context_summary` and `approach`
must be redacted before the durable event is committed.

## Consequences

- Qdrant remains a derived retrieval index, not the system of record.
- Read-only skill retrieval can ship while writes remain fail-closed.
- Enabling automatic learning requires a durable Core audit/outbox schema and a
  restricted executor; neither is smuggled into this change.
- Human confirmation cannot be inferred from capability tier or response text.
- Fine-tuning and weight changes remain outside this ADR.

## Rejected alternatives

- Reusing `jarvis_knowledge_bge_v1`: rejected because reviewed documents and
  learned experience have different trust, retention and revocation rules.
- Writing after any completed conversation: rejected because completion does
  not prove correctness or execution.
- Treating `authorization.approved` as success: rejected because authorization
  precedes execution and verification.
- Letting retrieved skills choose an alias or capability: rejected by ADR-014
  and enforced by typed model decisions in Core.
