# Canonical Wazuh Event v1 Proposal

Status: design only; not implemented
Principles: preserve Wazuh facts, explicit nulls, bounded values, no random/fabricated identifiers, backward-compatible projection.

## Contract

```json
{
  "schema_version": "wazuh.normalized.v1",
  "source": "wazuh",
  "alert_id": "string-or-null",
  "timestamp": "original-RFC3339-or-null",
  "timestamp_ms": 0,
  "received_at": "RFC3339",
  "agent": {
    "id": "string-or-null",
    "name": "string-or-null"
  },
  "rule": {
    "id": "string-or-null",
    "level": 0,
    "description": "string-or-null",
    "groups": null,
    "frequency": null
  },
  "mitre": [
    {
      "id": "T1059.001",
      "tactic": "Execution",
      "technique": "PowerShell"
    }
  ],
  "entities": {
    "host": "string-or-null",
    "src_user": "string-or-null",
    "dst_user": "string-or-null",
    "user": "compatibility-selected-user-or-null",
    "src_ip": "string-or-null",
    "dst_ip": "string-or-null",
    "process": "string-or-null",
    "parent_process": "string-or-null",
    "command_line": "string-or-null",
    "file": "string-or-null",
    "hash": {
      "algorithm": "sha256-or-null",
      "value": "string-or-null"
    }
  },
  "decoder": {
    "name": "string-or-null",
    "parent": "string-or-null"
  },
  "location": "string-or-null",
  "raw_reference": {
    "source": "wazuh-alerts-json",
    "alert_id": "string-or-null",
    "offset": null
  },
  "compatibility": {
    "id": "same-as-alert_id",
    "host": "same-as-entities.host",
    "severity": "critical|high|medium|low",
    "title": "same-as-rule.description",
    "description": "bounded-Wazuh-description-or-null",
    "source_ip": "same-as-entities.src_ip"
  }
}
```

## Null and derivation rules

- Missing source fields are JSON `null`; arrays are `null` when absent and `[]` only when Wazuh explicitly supplies an empty array.
- `alert_id` may not fall back to `rule.id`, random UUID or log text. An event without stable ID is accepted as incomplete evidence and gets a separate ingestion envelope ID, never presented as Wazuh alert ID.
- `timestamp_ms` is derived only from a valid original timestamp; otherwise null in the actual typed schema. `received_at` is Core ingestion time and is never substituted for occurrence time.
- `severity` is the only allowed deterministic projection from `rule.level`, using the current thresholds to preserve behavior. It is not stored as a Wazuh fact.
- `entities.user` is compatibility-only and should select `dst_user`, then `src_user`, without losing either original field.
- MITRE entries are built only from `rule.mitre`. Scalar/array source variants must be normalized without positional fabrication. If IDs and labels cannot be safely aligned, retain IDs with null labels and record missing information.
- `raw_reference` locates evidence; it must not embed an unbounded raw alert. Raw material remains behind an authorized bounded fetch path.

## Validation and bounds

- Reject unknown schema version and invalid types; tolerate missing optional Wazuh fields as null.
- Bound collection counts (MITRE/groups), individual strings and total serialized size using current Core limits.
- Validate IP/hash syntax only to classify quality; never drop the original bounded value silently.
- Preserve `agent.name` and `entities.host` separately even when initially identical.
- Evidence consumers cite `alert_id` plus event-envelope ID/raw reference.

## Compatibility rollout

1. Add the canonical object alongside current flat fields.
2. Make Core consume canonical first and flat fields second.
3. Update relay and agent with shared fixtures; update n8n only after live inventory.
4. Measure canonical/legacy parity in logs without raw content.
5. Remove legacy fields only in a future versioned release, not v0.2.

## Fixture plan

Create anonymized JSON under `tests/fixtures/wazuh/`: `high_no_mitre`, `powershell_benign`, `mitre_chain`, `duplicate`, `same_host_inside_30m`, `same_host_outside_30m`, `incomplete`, `mitre_multiple_arrays`, `critical_asset`, and `contradictory_evidence`. Fixtures must use reserved documentation IPs and synthetic hosts/users.
