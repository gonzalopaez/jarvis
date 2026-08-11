# ADR-013: Qdrant-backed infrastructure knowledge for conversation

## Status

Accepted — 2026-08-11.

## Decision

Jarvis Core retrieves bounded documentation context before calls to the
`jarvis-fast` and `jarvis-reasoning` LiteLLM aliases. Multilingual embeddings
are generated through the `jarvis-embed-multilingual` alias (Ollama `bge-m3`)
and searched in the dedicated Qdrant collection `jarvis_knowledge_bge_v1`.

LiteLLM remains the model gateway; Qdrant stores knowledge but does not train or
host the chat model. Live operational state continues to come from authenticated
tools and telemetry, never from the document index.

The index contains only reviewed repository documentation. It is separate from
`wazuh_alertas`, `threat_intel` and other existing collections. Every point has
bounded `text`, `source`, `title` and `sha256` payload fields. The ingestion
script stops if it detects a credential-shaped value.

Core uses a dedicated embeddings credential, an eight-second total retrieval
deadline, at most four results and a bounded 12 KiB context. Retrieved text is
explicitly treated as untrusted data, not instructions. Qdrant failure degrades
to the existing conversation path; it never blocks Core startup or enables an
action path.

## Operations

Index reviewed documentation with:

```text
scripts/rag-index.py . \
  --litellm-url http://<litellm>:4000 \
  --qdrant-url http://<qdrant>:6333 \
  --token-file /run/credentials/rag-ingest-token \
  --model jarvis-embed-multilingual \
  --collection jarvis_knowledge_bge_v1
```

Use `--recreate` only for this dedicated collection when published documents
were removed or changed; it prevents stale chunks from remaining searchable.

The runtime variables are documented in
`deploy/systemd/jarvis-core.env.example`. `rag-embeddings-token` is delivered as
a systemd credential and is never committed.
