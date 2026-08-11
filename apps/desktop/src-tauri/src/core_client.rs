use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    env, fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const API_VERSION: &str = "v1";
const MAX_MESSAGE_BYTES: usize = 8_000;
const MAX_TOKEN_BYTES: usize = 4_096;
const MIN_TOKEN_BYTES: usize = 32;
const MAX_RESPONSE_BYTES: usize = 64 * 1024;

static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub struct CoreClient {
    state: Result<ConfiguredClient, &'static str>,
}

struct ConfiguredClient {
    client: Client,
    endpoint: Url,
    token: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreHealth {
    pub online: bool,
    pub api_version: String,
    pub status: String,
    pub latency_ms: u128,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreConversation {
    pub request_id: String,
    pub status: String,
    pub audit_id: String,
    pub message: String,
    pub mode: String,
}

#[derive(Serialize)]
struct ConversationRequest<'a> {
    api_version: &'static str,
    request_id: &'a str,
    session_id: &'a str,
    kind: &'static str,
    message: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HealthResponse {
    api_version: String,
    status: String,
}

#[derive(Deserialize)]
struct CoreResponse {
    api_version: String,
    request_id: String,
    status: String,
    audit_id: String,
    data: Option<Value>,
}

impl CoreClient {
    pub fn from_environment() -> Self {
        Self {
            state: load_configuration(),
        }
    }

    pub async fn health(&self) -> Result<CoreHealth, &'static str> {
        let configured = self.configured()?;
        let started = Instant::now();
        let response = configured
            .client
            .get(route(&configured.endpoint, "v1/health")?)
            .send()
            .await
            .map_err(|_| "Core is unavailable")?;
        if response.status() != StatusCode::OK {
            return Err("Core health check was rejected");
        }
        let health: HealthResponse = bounded_json(response).await?;
        if health.api_version != API_VERSION || health.status != "ready" {
            return Err("Core health response is invalid");
        }
        Ok(CoreHealth {
            online: true,
            api_version: health.api_version,
            status: health.status,
            latency_ms: started.elapsed().as_millis(),
        })
    }

    pub async fn conversation(&self, message: String) -> Result<CoreConversation, &'static str> {
        let configured = self.configured()?;
        let message = message.trim();
        if message.is_empty() || message.len() > MAX_MESSAGE_BYTES {
            return Err("Conversation message is invalid");
        }
        let request_id = next_request_id();
        let request = ConversationRequest {
            api_version: API_VERSION,
            request_id: &request_id,
            session_id: &request_id,
            kind: "conversation",
            message,
        };
        let response = configured
            .client
            .post(route(&configured.endpoint, "v1/requests")?)
            .bearer_auth(&configured.token)
            .json(&request)
            .send()
            .await
            .map_err(|_| "Core is unavailable")?;
        if response.status() != StatusCode::OK {
            return Err("Core request was rejected");
        }
        let response: CoreResponse = bounded_json(response).await?;
        if response.api_version != API_VERSION
            || response.request_id != request_id
            || response.status != "completed"
        {
            return Err("Core response correlation failed");
        }
        let data = response
            .data
            .and_then(|value| value.as_object().cloned())
            .ok_or("Core response data is invalid")?;
        let message = data
            .get("message")
            .and_then(Value::as_str)
            .ok_or("Core response message is invalid")?;
        let mode = data
            .get("mode")
            .and_then(Value::as_str)
            .ok_or("Core response mode is invalid")?;
        Ok(CoreConversation {
            request_id: response.request_id,
            status: response.status,
            audit_id: response.audit_id,
            message: message.to_owned(),
            mode: mode.to_owned(),
        })
    }

    fn configured(&self) -> Result<&ConfiguredClient, &'static str> {
        self.state.as_ref().map_err(|message| *message)
    }
}

fn load_configuration() -> Result<ConfiguredClient, &'static str> {
    let endpoint = env::var("JARVIS_CORE_URL").map_err(|_| "Core endpoint is not configured")?;
    let endpoint = validate_endpoint(&endpoint)?;
    let token_path =
        env::var("JARVIS_CORE_TOKEN_FILE").map_err(|_| "Core credential is not configured")?;
    let token = load_token(Path::new(&token_path))?;
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none())
        .https_only(true)
        .build()
        .map_err(|_| "Core HTTPS client could not be created")?;
    Ok(ConfiguredClient {
        client,
        endpoint,
        token,
    })
}

fn validate_endpoint(value: &str) -> Result<Url, &'static str> {
    let url = Url::parse(value).map_err(|_| "Core endpoint is invalid")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err("Core endpoint must be an HTTPS origin");
    }
    Ok(url)
}

fn load_token(path: &Path) -> Result<String, &'static str> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "Core credential is unavailable")?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_TOKEN_BYTES as u64 + 2 {
        return Err("Core credential file is invalid");
    }
    let token = fs::read_to_string(path).map_err(|_| "Core credential could not be read")?;
    let token = token.trim();
    if token.len() < MIN_TOKEN_BYTES
        || token.len() > MAX_TOKEN_BYTES
        || token.bytes().any(|byte| !byte.is_ascii_graphic())
    {
        return Err("Core credential value is invalid");
    }
    Ok(token.to_owned())
}

fn route(endpoint: &Url, path: &str) -> Result<Url, &'static str> {
    endpoint
        .join(path)
        .map_err(|_| "Core endpoint route is invalid")
}

async fn bounded_json<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
) -> Result<T, &'static str> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err("Core response is too large");
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|_| "Core response could not be read")?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err("Core response is too large");
    }
    serde_json::from_slice(&bytes).map_err(|_| "Core response is invalid")
}

fn next_request_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let sequence = REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("desktop-{timestamp}-{sequence}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_requires_clean_https_origin() {
        assert!(validate_endpoint("https://jarvis.example.internal").is_ok());
        assert!(validate_endpoint("http://jarvis.example.internal").is_err());
        assert!(validate_endpoint("https://user@example.internal").is_err());
        assert!(validate_endpoint("https://example.internal/path").is_err());
    }

    #[test]
    fn route_stays_under_configured_origin() {
        let endpoint = validate_endpoint("https://jarvis.example.internal").expect("endpoint");
        assert_eq!(
            route(&endpoint, "v1/health").expect("route").as_str(),
            "https://jarvis.example.internal/v1/health"
        );
    }
}
