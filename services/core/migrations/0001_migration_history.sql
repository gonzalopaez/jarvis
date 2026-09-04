SET LOCAL lock_timeout = '2s';
SET LOCAL statement_timeout = '30s';

CREATE TABLE public.jarvis_schema_migrations (
    version text PRIMARY KEY,
    description text NOT NULL,
    checksum_sha256 text NOT NULL CHECK (checksum_sha256 ~ '^[0-9a-f]{64}$'),
    applied_at timestamp with time zone NOT NULL DEFAULT now(),
    applied_by text NOT NULL DEFAULT session_user
);
