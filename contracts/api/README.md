# API contracts

Version v1 is defined by JSON Schema:

- core-request.v1.schema.json
- core-response.v1.schema.json
- transport-error.v1.schema.json

Unknown fields are rejected. Conversation and action requests are mutually exclusive. Correlation IDs are caller-generated opaque identifiers with a restricted character set. Authentication is transport metadata and never part of the JSON body.

These schemas define the public envelope. Runtime validation in services/core is authoritative and has additional size, depth, policy and secret-field controls.

Transport errors omit correlation IDs when the request body could not be safely parsed. Error messages never reflect request content, credentials or parser details.
