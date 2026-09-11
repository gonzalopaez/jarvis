SET LOCAL lock_timeout = '2s';
SET LOCAL statement_timeout = '30s';

CREATE SCHEMA IF NOT EXISTS jarvis_core;

CREATE TABLE jarvis_core.schema_migrations (
    version text PRIMARY KEY,
    checksum_sha256 text NOT NULL CHECK (checksum_sha256 ~ '^[0-9a-f]{64}$'),
    applied_at timestamp with time zone NOT NULL DEFAULT now()
);

CREATE TABLE jarvis_core.audit_events (
    audit_id text PRIMARY KEY,
    request_id text NOT NULL,
    subject text NOT NULL,
    capability text NOT NULL,
    capability_tier smallint NOT NULL CHECK (capability_tier BETWEEN 1 AND 3),
    outcome text NOT NULL CHECK (outcome IN ('verified', 'verification_failed')),
    provenance jsonb NOT NULL,
    created_at timestamp with time zone NOT NULL DEFAULT now()
);

CREATE TABLE jarvis_core.outbox_events (
    event_id text PRIMARY KEY,
    event_type text NOT NULL CHECK (event_type = 'task_outcome.verified.v1'),
    audit_id text NOT NULL REFERENCES jarvis_core.audit_events(audit_id),
    payload jsonb NOT NULL,
    state text NOT NULL DEFAULT 'pending'
        CHECK (state IN ('pending', 'processing', 'delivered', 'failed')),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    next_attempt_at timestamp with time zone NOT NULL DEFAULT now(),
    locked_until timestamp with time zone,
    last_error text,
    created_at timestamp with time zone NOT NULL DEFAULT now(),
    processed_at timestamp with time zone
);

CREATE INDEX core_outbox_claim_idx
    ON jarvis_core.outbox_events (next_attempt_at, created_at)
    WHERE state IN ('pending', 'processing');
