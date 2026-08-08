# ADR-007: Separate LLM and executor

- Status: Accepted
- Date: 2026-08-08

## Decision

LLM output cannot execute commands. Actions cross schema validation, policy, authorization and a restricted capability-specific executor.

## Consequences

Arbitrary shell tools are prohibited. Results require verification and audit; credentials are brokered outside model context.
