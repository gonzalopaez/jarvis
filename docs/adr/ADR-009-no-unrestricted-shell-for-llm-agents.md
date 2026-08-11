# ADR-009: No unrestricted shell for LLM agents

Status: accepted

Codex and other models are untrusted proposers, not executors. Production agents receive no arbitrary endpoint shell. Read-only milestone analysis runs in an isolated local sandbox. Future host operations must use schema-bound semantic MCP tools, deny-by-default policy, explicit authorization for modification, verified results and separate audit records.
