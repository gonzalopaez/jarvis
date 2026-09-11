use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalWazuhEvent {
    pub schema_version: String,
    pub source: String,
    pub alert_id: Option<String>,
    pub timestamp: Option<String>,
    pub timestamp_ms: Option<u64>,
    pub received_at: String,
    pub agent: WazuhAgentIdentity,
    pub rule: WazuhRule,
    pub mitre: Option<Vec<MitreReference>>,
    pub entities: WazuhEntities,
    pub decoder: Option<WazuhDecoder>,
    pub location: Option<String>,
    pub raw_reference: Value,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WazuhAgentIdentity {
    pub id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WazuhRule {
    pub id: Option<String>,
    pub level: Option<u8>,
    pub description: Option<String>,
    pub groups: Option<Vec<String>>,
    pub frequency: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MitreReference {
    pub id: String,
    pub tactic: Option<String>,
    pub technique: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WazuhEntities {
    pub host: Option<String>,
    pub src_user: Option<String>,
    pub dst_user: Option<String>,
    pub user: Option<String>,
    pub src_ip: Option<String>,
    pub dst_ip: Option<String>,
    pub process: Option<String>,
    pub parent_process: Option<String>,
    pub command_line: Option<String>,
    pub file: Option<String>,
    pub hash: Option<WazuhHash>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WazuhHash {
    pub algorithm: Option<String>,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WazuhDecoder {
    pub name: Option<String>,
    pub parent: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_fields_remain_null_and_mitre_is_not_inferred() {
        let event: CanonicalWazuhEvent = serde_json::from_value(serde_json::json!({
            "schema_version":"wazuh.normalized.v1","source":"wazuh","alert_id":null,"timestamp":null,
            "timestamp_ms":null,"received_at":"2026-01-01T00:00:00Z","agent":{"id":null,"name":null},
            "rule":{"id":null,"level":13,"description":null,"groups":null,"frequency":null},"mitre":null,
            "entities":{"host":null,"src_user":null,"dst_user":null,"user":null,"src_ip":null,"dst_ip":null,
            "process":null,"parent_process":null,"command_line":null,"file":null,"hash":null},
            "decoder":null,"location":null,"raw_reference":{"source":"wazuh-alerts-json","alert_id":null}
        })).unwrap();
        assert!(event.alert_id.is_none());
        assert!(event.agent.id.is_none());
        assert!(event.mitre.is_none());
    }
}
