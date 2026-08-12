# n8n integration

The production `SOC 2.0` workflow is intentionally mechanical. It accepts a
Wazuh webhook, normalizes the alert, correlates by host and user in a bounded
five-minute window, and sends a Telegram notification. It contains no LLM
triage and no `ACCIÓN:*` nodes; triage and proposals belong to the Wazuh Agent,
while authorization belongs to Core.

`SOC_2_0_correlation.json` is the sanitized source template. Credential values,
credential IDs, workflow IDs, chat IDs, and internal addresses must not be
committed.

The deployed workflow was exercised with a real Wazuh alert after publication;
webhook, normalization, correlation, and notification all completed
successfully.
