# Secrets management

OpenBao is the central secret-management boundary. It stores passwords, API keys, tokens, service credentials and future dynamic credentials/PKI material.

Git contains code, documentation, schemas, sanitized workflows and non-sensitive examples only. Each workload receives a unique identity and the smallest required path/capabilities. Root tokens and shared administrative identities are prohibited in applications.

Secrets must never become model context. A model requests a capability; a credential broker retrieves and uses credentials outside the model boundary. Audit records identify the actor, capability and result without recording secret values.

OpenBao integration is not implemented in v0.1-clean.
