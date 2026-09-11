use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DOCUMENT_KNOWLEDGE_COLLECTION: &str = "jarvis_knowledge_bge_v1";
const MAX_QUERY_BYTES: usize = 8 * 1024;
const MAX_UPSTREAM_BYTES: usize = 2 * 1024 * 1024;
const MAX_CONTEXT_BYTES: usize = 8 * 1024;
const MAX_VECTOR_DIMENSIONS: usize = 8 * 1024;
const MAX_RESULTS: usize = 3;
const SKILL_RETENTION_SECONDS: u64 = 180 * 24 * 60 * 60;
const CAPABILITIES_JSON: &str = include_str!("../../../contracts/data/capabilities.json");

#[derive(Clone)]
pub struct SkillMemoryClient {
    client: Client,
    config: SkillMemoryConfig,
}

#[derive(Clone)]
pub struct SkillMemoryConfig {
    pub litellm_base_url: Url,
    pub litellm_token: String,
    pub embedding_model: String,
    pub qdrant_base_url: Url,
    pub collection: String,
    pub score_threshold: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillMemoryError {
    InvalidConfiguration,
    Unavailable,
    InvalidResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskOutcomeProvenance {
    pub kind: String,
    pub adapter: String,
    pub response_schema: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommittedTaskOutcomeRecord {
    pub schema_version: String,
    pub source_event_id: String,
    pub source_audit_id: String,
    pub source_request_id: String,
    pub subject: String,
    pub task_type: String,
    pub context_summary: String,
    pub approach: String,
    pub capability: String,
    pub capability_tier: u8,
    pub executor_verified: bool,
    pub human_authorization_audit_id: Option<String>,
    pub provenance: TaskOutcomeProvenance,
    pub created_at: String,
    pub committed_at_epoch_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedTaskOutcome {
    source_event_id: String,
    source_audit_id: String,
    task_type: String,
    context_summary: String,
    approach: String,
    capability: String,
    capability_tier: u8,
    human_confirmed: bool,
    created_at: String,
    created_at_epoch_seconds: u64,
}

impl VerifiedTaskOutcome {
    pub fn from_committed_record(
        record: CommittedTaskOutcomeRecord,
    ) -> Result<Self, SkillMemoryError> {
        let human_confirmed = record.human_authorization_audit_id.is_some();
        if record.schema_version != "task_outcome.verified.v1"
            || !record.executor_verified
            || record.committed_at_epoch_seconds == 0
            || !valid_task_type(&record.task_type)
            || !valid_identifier(&record.source_event_id, 160)
            || !valid_identifier(&record.source_audit_id, 160)
            || !valid_identifier(&record.source_request_id, 160)
            || !valid_identifier(&record.subject, 160)
            || !valid_identifier(&record.capability, 160)
            || !(1..=3).contains(&record.capability_tier)
            || catalog_tier(&record.capability) != Some(record.capability_tier)
            || (record.capability_tier >= 2 && !human_confirmed)
            || record
                .human_authorization_audit_id
                .as_deref()
                .is_some_and(|value| !valid_identifier(value, 160))
            || !valid_identifier(&record.provenance.adapter, 96)
            || !valid_identifier(&record.provenance.response_schema, 96)
            || (record.capability_tier == 1 && record.provenance.kind != "validated_read_adapter")
            || (record.capability_tier >= 2 && record.provenance.kind != "restricted_executor")
            || !valid_text(&record.context_summary, 2 * 1024)
            || !valid_text(&record.approach, 4 * 1024)
            || !valid_text(&record.created_at, 64)
        {
            return Err(SkillMemoryError::InvalidResponse);
        }
        Ok(Self {
            source_event_id: record.source_event_id,
            source_audit_id: record.source_audit_id,
            task_type: record.task_type,
            context_summary: record.context_summary,
            approach: record.approach,
            capability: record.capability,
            capability_tier: record.capability_tier,
            human_confirmed,
            created_at: record.created_at,
            created_at_epoch_seconds: record.committed_at_epoch_seconds,
        })
    }
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

#[derive(Deserialize)]
struct SearchResponse {
    result: Vec<SkillHit>,
}

#[derive(Deserialize)]
struct SkillHit {
    score: f64,
    payload: SkillPayload,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SkillPayload {
    schema_version: String,
    skill_id: String,
    task_type: String,
    context_summary: String,
    approach: String,
    outcome: String,
    capability: String,
    capability_tier: u8,
    human_confirmed: bool,
    source_event_id: String,
    source_audit_id: String,
    created_at: String,
    expires_at_epoch_seconds: u64,
    revoked: bool,
}

impl SkillMemoryClient {
    pub fn new(config: SkillMemoryConfig) -> Result<Self, SkillMemoryError> {
        if config.litellm_base_url.scheme() != "http"
            || config.qdrant_base_url.scheme() != "http"
            || config.litellm_token.len() < 20
            || config.embedding_model.trim().is_empty()
            || !valid_collection(&config.collection)
            || config.collection == DOCUMENT_KNOWLEDGE_COLLECTION
            || !config.score_threshold.is_finite()
            || !(0.0..=1.0).contains(&config.score_threshold)
        {
            return Err(SkillMemoryError::InvalidConfiguration);
        }
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(8))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| SkillMemoryError::InvalidConfiguration)?;
        Ok(Self { client, config })
    }

    pub async fn retrieve(
        &self,
        task_type: &str,
        query: &str,
    ) -> Result<Option<String>, SkillMemoryError> {
        let task_type = task_type.trim();
        let query = query.trim();
        if !valid_task_type(task_type) || query.is_empty() || query.len() > MAX_QUERY_BYTES {
            return Err(SkillMemoryError::InvalidResponse);
        }
        let vector = self.embed(query).await?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SkillMemoryError::InvalidResponse)?
            .as_secs();
        let url = self
            .config
            .qdrant_base_url
            .join(&format!(
                "collections/{}/points/search",
                self.config.collection
            ))
            .map_err(|_| SkillMemoryError::InvalidConfiguration)?;
        let response = self
            .client
            .post(url)
            .json(&json!({
                "vector": vector,
                "limit": MAX_RESULTS,
                "score_threshold": self.config.score_threshold,
                "filter": {"must": [
                    {"key": "schema_version", "match": {"value": "jarvis.skill.v1"}},
                    {"key": "task_type", "match": {"value": task_type}},
                    {"key": "outcome", "match": {"value": "success"}},
                    {"key": "revoked", "match": {"value": false}},
                    {"key": "expires_at_epoch_seconds", "range": {"gt": now}}
                ]},
                "with_payload": true
            }))
            .send()
            .await
            .map_err(|_| SkillMemoryError::Unavailable)?;
        if !response.status().is_success() {
            return Err(SkillMemoryError::Unavailable);
        }
        let body: SearchResponse = bounded_json(response).await?;
        render_context(body.result, task_type, now)
    }

    pub async fn write_verified(
        &self,
        outcome: &VerifiedTaskOutcome,
    ) -> Result<(), SkillMemoryError> {
        let retrieval_document = format!(
            "{}\n{}\n{}",
            outcome.task_type, outcome.context_summary, outcome.approach
        );
        let vector = self.embed(&retrieval_document).await?;
        let skill_id = deterministic_point_id(&outcome.source_event_id);
        let expires_at_epoch_seconds = outcome
            .created_at_epoch_seconds
            .checked_add(SKILL_RETENTION_SECONDS)
            .ok_or(SkillMemoryError::InvalidResponse)?;
        let payload = SkillPayload {
            schema_version: "jarvis.skill.v1".into(),
            skill_id: skill_id.clone(),
            task_type: outcome.task_type.clone(),
            context_summary: outcome.context_summary.clone(),
            approach: outcome.approach.clone(),
            outcome: "success".into(),
            capability: outcome.capability.clone(),
            capability_tier: outcome.capability_tier,
            human_confirmed: outcome.human_confirmed,
            source_event_id: outcome.source_event_id.clone(),
            source_audit_id: outcome.source_audit_id.clone(),
            created_at: outcome.created_at.clone(),
            expires_at_epoch_seconds,
            revoked: false,
        };
        let url = self
            .config
            .qdrant_base_url
            .join(&format!(
                "collections/{}/points?wait=true",
                self.config.collection
            ))
            .map_err(|_| SkillMemoryError::InvalidConfiguration)?;
        let response = self
            .client
            .put(url)
            .json(&json!({"points": [{"id": skill_id, "vector": vector, "payload": payload}]}))
            .send()
            .await
            .map_err(|_| SkillMemoryError::Unavailable)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(SkillMemoryError::Unavailable)
        }
    }

    async fn embed(&self, query: &str) -> Result<Vec<f32>, SkillMemoryError> {
        let url = self
            .config
            .litellm_base_url
            .join("v1/embeddings")
            .map_err(|_| SkillMemoryError::InvalidConfiguration)?;
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.config.litellm_token)
            .json(&json!({"model": self.config.embedding_model, "input": query}))
            .send()
            .await
            .map_err(|_| SkillMemoryError::Unavailable)?;
        if !response.status().is_success() {
            return Err(SkillMemoryError::Unavailable);
        }
        let mut body: EmbeddingResponse = bounded_json(response).await?;
        if body.data.len() != 1 {
            return Err(SkillMemoryError::InvalidResponse);
        }
        let vector = body.data.remove(0).embedding;
        if vector.is_empty()
            || vector.len() > MAX_VECTOR_DIMENSIONS
            || vector.iter().any(|value| !value.is_finite())
        {
            return Err(SkillMemoryError::InvalidResponse);
        }
        Ok(vector)
    }
}

async fn bounded_json<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
) -> Result<T, SkillMemoryError> {
    let bytes = response
        .bytes()
        .await
        .map_err(|_| SkillMemoryError::InvalidResponse)?;
    if bytes.len() > MAX_UPSTREAM_BYTES {
        return Err(SkillMemoryError::InvalidResponse);
    }
    serde_json::from_slice(&bytes).map_err(|_| SkillMemoryError::InvalidResponse)
}

fn render_context(
    hits: Vec<SkillHit>,
    requested_task_type: &str,
    now: u64,
) -> Result<Option<String>, SkillMemoryError> {
    let mut output = String::new();
    for hit in hits {
        let payload = hit.payload;
        if !hit.score.is_finite()
            || payload.schema_version != "jarvis.skill.v1"
            || payload.task_type != requested_task_type
            || payload.outcome != "success"
            || payload.revoked
            || payload.expires_at_epoch_seconds <= now
            || !(1..=3).contains(&payload.capability_tier)
            || (payload.capability_tier >= 2 && !payload.human_confirmed)
            || !valid_identifier(&payload.skill_id, 160)
            || !valid_identifier(&payload.capability, 160)
            || !valid_identifier(&payload.source_event_id, 160)
            || !valid_identifier(&payload.source_audit_id, 160)
            || !valid_text(&payload.context_summary, 2 * 1024)
            || !valid_text(&payload.approach, 4 * 1024)
            || !valid_text(&payload.created_at, 64)
        {
            return Err(SkillMemoryError::InvalidResponse);
        }
        let rendered = format!(
            "EXPERIENCIA HISTÓRICA NO CONFIABLE [{}]\nContexto: {}\nPrincipio reutilizable: {}\n",
            payload.skill_id, payload.context_summary, payload.approach
        );
        if output.len() + rendered.len() > MAX_CONTEXT_BYTES {
            break;
        }
        output.push_str(&rendered);
    }
    if output.is_empty() {
        Ok(None)
    } else {
        Ok(Some(output.trim().to_owned()))
    }
}

fn valid_collection(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_task_type(value: &str) -> bool {
    valid_identifier(value, 128)
}

fn valid_identifier(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'/')
        })
}

fn valid_text(value: &str, max: usize) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.len() <= max
        && !value.chars().any(char::is_control)
        && !contains_secret_shape(value)
}

fn contains_secret_shape(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "authorization:",
        "bearer ",
        "api_key",
        "password=",
        "token=",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn catalog_tier(capability: &str) -> Option<u8> {
    let entries: serde_json::Value = serde_json::from_str(CAPABILITIES_JSON).ok()?;
    entries.as_array()?.iter().find_map(|entry| {
        (entry.get("capability")?.as_str()? == capability)
            .then(|| {
                entry
                    .get("tier")?
                    .as_u64()
                    .and_then(|tier| u8::try_from(tier).ok())
            })
            .flatten()
    })
}

fn deterministic_point_id(source_event_id: &str) -> String {
    let digest = hex::encode(Sha256::digest(source_event_id.as_bytes()));
    format!(
        "{}-{}-{}-{}-{}",
        &digest[0..8],
        &digest[8..12],
        &digest[12..16],
        &digest[16..20],
        &digest[20..32]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(tier: u8, confirmed: bool) -> SkillPayload {
        SkillPayload {
            schema_version: "jarvis.skill.v1".into(),
            skill_id: "skill-1".into(),
            task_type: "wazuh_alert_triage".into(),
            context_summary: "Alertas correlacionadas por host y ventana temporal".into(),
            approach: "Agrupar primero por host y después validar evidencia independiente".into(),
            outcome: "success".into(),
            capability: "wazuh.alerts.read".into(),
            capability_tier: tier,
            human_confirmed: confirmed,
            source_event_id: "event-1".into(),
            source_audit_id: "audit-1".into(),
            created_at: "2026-09-11T12:00:00-03:00".into(),
            expires_at_epoch_seconds: 2_000_000_000,
            revoked: false,
        }
    }

    fn committed_record(tier: u8, authorization: Option<&str>) -> CommittedTaskOutcomeRecord {
        let capability = match tier {
            1 => "wazuh.alerts.read",
            2 => "security.ip.block",
            3 => "proxmox.vm.deploy",
            _ => "unsupported",
        };
        CommittedTaskOutcomeRecord {
            schema_version: "task_outcome.verified.v1".into(),
            source_event_id: "event-verified-1".into(),
            source_audit_id: "audit-verified-1".into(),
            source_request_id: "request-verified-1".into(),
            subject: "operator".into(),
            task_type: "wazuh_alert_triage".into(),
            context_summary: "Alertas correlacionadas por host".into(),
            approach: "Agrupar por host antes de evaluar severidad".into(),
            capability: capability.into(),
            capability_tier: tier,
            executor_verified: true,
            human_authorization_audit_id: authorization.map(str::to_owned),
            provenance: TaskOutcomeProvenance {
                kind: if tier == 1 {
                    "validated_read_adapter"
                } else {
                    "restricted_executor"
                }
                .into(),
                adapter: "test-adapter".into(),
                response_schema: "test.response.v1".into(),
            },
            created_at: "2026-09-11T12:00:00-03:00".into(),
            committed_at_epoch_seconds: 1_789_136_400,
        }
    }

    #[test]
    fn renders_only_bounded_active_matching_skills() {
        let context = render_context(
            vec![SkillHit {
                score: 0.9,
                payload: payload(1, false),
            }],
            "wazuh_alert_triage",
            1_800_000_000,
        )
        .expect("valid skill")
        .expect("context");
        assert!(context.contains("EXPERIENCIA HISTÓRICA NO CONFIABLE"));
        assert!(context.contains("Principio reutilizable"));
        assert!(!context.contains("source_event_id"));
    }

    #[test]
    fn rejects_unconfirmed_tier_two_skill() {
        assert_eq!(
            render_context(
                vec![SkillHit {
                    score: 0.9,
                    payload: payload(2, false),
                }],
                "wazuh_alert_triage",
                1_800_000_000,
            ),
            Err(SkillMemoryError::InvalidResponse)
        );
    }

    #[test]
    fn documentary_collection_cannot_be_reused() {
        let config = SkillMemoryConfig {
            litellm_base_url: "http://litellm.internal/".parse().unwrap(),
            litellm_token: "l".repeat(20),
            embedding_model: "jarvis-embed-multilingual".into(),
            qdrant_base_url: "http://qdrant.internal/".parse().unwrap(),
            collection: DOCUMENT_KNOWLEDGE_COLLECTION.into(),
            score_threshold: 0.6,
        };
        assert!(matches!(
            SkillMemoryClient::new(config),
            Err(SkillMemoryError::InvalidConfiguration)
        ));
    }

    #[test]
    fn verified_outcome_requires_durable_execution_and_tier_authorization() {
        let mut unverified = committed_record(1, None);
        unverified.executor_verified = false;
        assert_eq!(
            VerifiedTaskOutcome::from_committed_record(unverified),
            Err(SkillMemoryError::InvalidResponse)
        );
        assert_eq!(
            VerifiedTaskOutcome::from_committed_record(committed_record(2, None)),
            Err(SkillMemoryError::InvalidResponse)
        );
        assert!(VerifiedTaskOutcome::from_committed_record(committed_record(
            2,
            Some("authorization-audit-1")
        ))
        .is_ok());
    }

    #[test]
    fn point_identifier_is_stable_and_qdrant_compatible() {
        let first = deterministic_point_id("event-verified-1");
        assert_eq!(first, deterministic_point_id("event-verified-1"));
        assert_eq!(first.len(), 36);
        assert_eq!(first.chars().filter(|value| *value == '-').count(), 4);
    }
}
