# Event contracts

audit-event.v1.schema.json defines the first sanitized Core audit event. It intentionally excludes messages, action parameters, headers, credentials, model prompts and raw executor output.

realtime-envelope.v1.schema.json defines the normalized bounded envelope sent through the authenticated WebSocket gateway. Initial connections receive `system.snapshot`; heartbeats preserve liveness and lagged subscribers receive `system.resync_required` followed by a fresh snapshot.

telemetry-snapshot.v1.schema.json defines normalized operational metrics. Source-specific Prometheus labels and Wazuh documents never cross this boundary.
`task-outcome-verified.v1.schema.json` is the sole durable automatic skill-write
input. In Phase 1.5 only validated Tier 1 read-adapter outcomes may be consumed;
Tier 2/3 remain disabled and continue to require RestrictedExecutor plus durable
human authorization.
