use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

const MAX_QUERY_BYTES: usize = 8 * 1024;
const MAX_UPSTREAM_BYTES: usize = 2 * 1024 * 1024;
const MAX_CONTEXT_BYTES: usize = 12 * 1024;
const MAX_VECTOR_DIMENSIONS: usize = 8 * 1024;

#[derive(Clone)]
pub struct KnowledgeClient {
    client: Client,
    config: KnowledgeConfig,
}

#[derive(Clone)]
pub struct KnowledgeConfig {
    pub litellm_base_url: Url,
    pub litellm_token: String,
    pub embedding_model: String,
    pub qdrant_base_url: Url,
    pub collection: String,
    pub limit: usize,
    pub score_threshold: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnowledgeError {
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
    result: Vec<SearchHit>,
}

#[derive(Deserialize)]
struct SearchHit {
    score: f64,
    payload: KnowledgePayload,
}

#[derive(Deserialize)]
struct KnowledgePayload {
    text: String,
    source: String,
    #[serde(default)]
    title: String,
}

impl KnowledgeClient {
    pub fn new(config: KnowledgeConfig) -> Result<Self, KnowledgeError> {
        if config.litellm_base_url.scheme() != "http"
            || config.qdrant_base_url.scheme() != "http"
            || config.litellm_token.len() < 20
            || config.embedding_model.trim().is_empty()
            || !valid_collection(&config.collection)
            || !(1..=8).contains(&config.limit)
            || !config.score_threshold.is_finite()
            || !(0.0..=1.0).contains(&config.score_threshold)
        {
            return Err(KnowledgeError::InvalidConfiguration);
        }
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(8))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| KnowledgeError::InvalidConfiguration)?;
        Ok(Self { client, config })
    }

    pub async fn retrieve(&self, query: &str) -> Result<Option<String>, KnowledgeError> {
        let query = query.trim();
        if query.is_empty() || query.len() > MAX_QUERY_BYTES {
            return Err(KnowledgeError::InvalidResponse);
        }
        let vector = self.embed(query).await?;
        let url = self
            .config
            .qdrant_base_url
            .join(&format!(
                "collections/{}/points/search",
                self.config.collection
            ))
            .map_err(|_| KnowledgeError::InvalidConfiguration)?;
        let response = self
            .client
            .post(url)
            .json(&json!({
                "vector": vector,
                "limit": self.config.limit,
                "score_threshold": self.config.score_threshold,
                "with_payload": ["text", "source", "title"]
            }))
            .send()
            .await
            .map_err(|_| KnowledgeError::Unavailable)?;
        if !response.status().is_success() {
            return Err(KnowledgeError::Unavailable);
        }
        let body: SearchResponse = bounded_json(response).await?;
        render_context(body.result)
    }

    async fn embed(&self, query: &str) -> Result<Vec<f32>, KnowledgeError> {
        let url = self
            .config
            .litellm_base_url
            .join("v1/embeddings")
            .map_err(|_| KnowledgeError::InvalidConfiguration)?;
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.config.litellm_token)
            .json(&json!({"model": self.config.embedding_model, "input": query}))
            .send()
            .await
            .map_err(|_| KnowledgeError::Unavailable)?;
        if !response.status().is_success() {
            return Err(KnowledgeError::Unavailable);
        }
        let mut body: EmbeddingResponse = bounded_json(response).await?;
        if body.data.len() != 1 {
            return Err(KnowledgeError::InvalidResponse);
        }
        let vector = body.data.remove(0).embedding;
        if vector.is_empty()
            || vector.len() > MAX_VECTOR_DIMENSIONS
            || vector.iter().any(|value| !value.is_finite())
        {
            return Err(KnowledgeError::InvalidResponse);
        }
        Ok(vector)
    }
}

async fn bounded_json<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
) -> Result<T, KnowledgeError> {
    let bytes = response
        .bytes()
        .await
        .map_err(|_| KnowledgeError::InvalidResponse)?;
    if bytes.len() > MAX_UPSTREAM_BYTES {
        return Err(KnowledgeError::InvalidResponse);
    }
    serde_json::from_slice(&bytes).map_err(|_| KnowledgeError::InvalidResponse)
}

fn render_context(hits: Vec<SearchHit>) -> Result<Option<String>, KnowledgeError> {
    let mut output = String::new();
    for hit in hits {
        let text = hit.payload.text.trim();
        let source = hit.payload.source.trim();
        if !hit.score.is_finite()
            || text.is_empty()
            || source.is_empty()
            || text.chars().any(|c| c == '\0')
            || source.chars().any(char::is_control)
        {
            return Err(KnowledgeError::InvalidResponse);
        }
        let title = hit.payload.title.trim();
        let header = if title.is_empty() {
            format!("FUENTE: [{source}]\n")
        } else {
            format!("FUENTE: [{source}] — {title}\n")
        };
        if output.len() + header.len() + text.len() + 2 > MAX_CONTEXT_BYTES {
            break;
        }
        output.push_str(&header);
        output.push_str(text);
        output.push_str("\n\n");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_bounded_context_with_sources() {
        let context = render_context(vec![SearchHit {
            score: 0.91,
            payload: KnowledgePayload {
                text: "Core se ejecuta en CT124.".into(),
                source: "docs/architecture.md".into(),
                title: "Arquitectura".into(),
            },
        }])
        .expect("valid context")
        .expect("context present");
        assert!(context.contains("[docs/architecture.md]"));
        assert!(context.contains("CT124"));
    }

    #[test]
    fn rejects_unsafe_collection_names() {
        assert!(!valid_collection("../secrets"));
        assert!(valid_collection("jarvis_knowledge_v1"));
    }
}
