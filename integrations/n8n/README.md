# n8n integration

The production `SOC 2.0` workflow is intentionally mechanical. It accepts a
Wazuh webhook, normalizes the alert, correlates by host and user in a bounded
five-minute window, and sends a Telegram notification. It contains no LLM
triage and no `ACCIÓN:*` nodes; triage and proposals belong to the Wazuh Agent,
while authorization belongs to Core.

Wazuh Agent forwards allow-listed `proposed_actions` to Core without attaching
its own authorization. Core applies the tiered policy and human-authorization
boundary. This is covered by
`test_proposal_reaches_core_as_action_and_is_not_executed_by_agent` and
`domain_agent_cannot_submit_human_confirmation` in baseline `a2f37e0`.

`SOC_2_0_correlation.json` is the sanitized source template. Credential values,
credential IDs, workflow IDs, chat IDs, and internal addresses must not be
committed.

The deployed workflow was exercised with a real Wazuh alert after publication;
webhook, normalization, correlation, and notification all completed
successfully.
