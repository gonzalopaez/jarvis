# Realtime architecture

The server Event Bus, `/ws` gateway and browser realtime client are implemented. The browser connects only through the same-origin route exposed by Nginx and never connects directly to Prometheus, Wazuh, Codex, MCP or internal service addresses. It probes its HttpOnly session before opening the socket and does not retry when authentication is absent.

The gateway sends a bounded versioned snapshot followed by normalized incremental events. Every envelope carries an event type, schema version, event identifier, timestamp and correlation identifier where applicable.

Implemented controls:

- authentication before upgrade;
- exact configured HTTPS Origin comparison;
- disabled-by-default gateway when no Origin is configured;
- bounded broadcast queue;
- read-only client channel with a 16 KiB inbound ceiling;
- 25-second heartbeat;
- lag detection followed by `system.resync_required` and a fresh snapshot;
- HTTP connection upgrades enabled only on the private Core listener.

The Web UI validates bounded envelopes, applies snapshots and normalized state/agent events, reconnects with exponential backoff plus jitter and suspends the socket while the tab is hidden. Invalid or oversized events close the connection. No unauthenticated fallback is permitted.

The frontend keeps a typed in-process Event Bus solely as a presentation boundary. The server Event Bus uses a bounded Tokio broadcast channel and is the source for WebSocket delivery.

Operational telemetry uses `telemetry.snapshot`; source health uses `telemetry.source.status`. The Web UI maps these events to telemetry widgets and Agent Matrix without starting a browser polling loop.
