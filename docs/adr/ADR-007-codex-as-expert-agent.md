# ADR-007: Codex as an expert agent

Status: accepted

JARVIS uses Codex only for technical and development work. Ordinary conversation continues through LiteLLM. The integration uses the official Codex SDK behind an internal task service rather than parsing terminal output. This preserves a small Core contract, thread continuity, timeouts and circuit-breaker semantics.
