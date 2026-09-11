# Core database migrations

These migrations target the dedicated Core database, never `jarvis_soc`.
They are not executed automatically at startup. Application credentials must
not own schema migration privileges.

The outbox worker claims work with `FOR UPDATE SKIP LOCKED` in application SQL;
the migration provides the unique event key and partial claim index.
