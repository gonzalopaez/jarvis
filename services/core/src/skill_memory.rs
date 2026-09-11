use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::json;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DOCUMENT_KNOWLEDGE_COLLECTION: &str = "jarvis_knowledge_bge_v1";
const MAX_QUERY_BYTES: usize = 8 * 1024;
const MAX_UPSTREAM_BYTES: usize = 2 * 1024 * 1024;
const MAX_CONTEXT_BYTES: usize = 8 * 1024;
const MAX_VECTOR_DIMENSIONS: usize = 8 * 1024;
const MAX_RESULTS: usize = 3;

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

#[derive(Deserialize)]
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
}
