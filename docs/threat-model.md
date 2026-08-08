# Threat model

| Threat | Primary controls |
| --- | --- |
| Prompt injection and malicious model output | Treat output as untrusted; schemas, policy, authorization and restricted executors |
| Credential disclosure | OpenBao, brokered use, redaction, short-lived identities and no model context |
| Arbitrary command execution | No direct LLM-to-shell; allow-listed structured capabilities |
| Compromised Desktop | Single low-privilege Core API; no backend addresses or infrastructure credentials |
| Compromised service | Network segmentation, unique identity, minimal policy and rapid revocation |
| Workflow/export leakage | Sanitized n8n exports and secret scanning |
| Supply-chain compromise | Locked dependencies, review, CI scanning and signed artifacts in future |
| Network interception | HTTPS/WSS and mTLS where appropriate |
| Audit tampering or gaps | Structured append-oriented audit with correlation and verification |

Trust-boundary and abuse-case reviews are required before each external integration.
