# Security architecture

The governing flow is:

    LLM / Agent -> Structured Request -> Schema Validation -> Policy Engine
    -> Authorization -> Restricted Executor -> Credential Broker -> OpenBao
    -> Target -> Verification -> Audit

Models propose; policy and authorization decide; restricted code executes. Executors expose allow-listed capabilities rather than arbitrary commands. Credentials remain behind the broker boundary and are never returned to models.

Controls include least-privilege identities, deny-by-default policies, input/output schemas, timeouts, bounded payloads, sanitized logs, correlation IDs, explicit approval for high-impact actions and postcondition verification.
