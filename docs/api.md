# Jarvis Core API

Jarvis Core will provide the Desktop single versioned HTTPS/WSS API. The initial contract will include health, telemetry/event delivery, conversational requests and structured-action status.

Requests require bounded payloads, schema validation, authentication, authorization, correlation IDs and safe error envelopes. Internal routing details and downstream credentials are never exposed. Concrete schemas will be added under contracts/api before Core implementation.
