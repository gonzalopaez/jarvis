use jarvis_core::{
    bind_private, run_prometheus_availability_until, serve_until, ActionRequest, AgentHealthCheck,
    AgentHealthPoller, AuditEvent, AuditSink, BearerAuthenticator, CodexHttpClient,
    ConversationService, CoreGateway, CredentialRecord, EventBus, ExecutionResult, PolicyEngine,
    Principal, PrometheusTelemetryAdapter, RestrictedExecutor, TelemetryService, Transport,
    TransportConfig, VoicePipeline, VoicePipelineConfig, WazuhSecurityPoller,
    DEFAULT_TELEMETRY_INTERVAL,
};
use reqwest::Url;
use serde::Deserialize;
use serde_json::json;
use std::{env, fs, net::SocketAddr, path::Path, sync::Arc};

const CREDENTIAL_NAME: &str = "auth-registry.json";
const MAX_CREDENTIAL_FILE_BYTES: u64 = 64 * 1024;
const VOICE_CREDENTIAL_NAME: &str = "voice-service-token";
const LITELLM_CREDENTIAL_NAME: &str = "litellm-token";
const CODEX_CREDENTIAL_NAME: &str = "codex-service-token";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialRegistry {
    credentials: Vec<CredentialEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialEntry {
    sha256: String,
    subject: String,
    roles: Vec<String>,
}

struct DisabledExecutor;

impl RestrictedExecutor for DisabledExecutor {
    fn execute(
        &self,
        _principal: &Principal,
        _action: &ActionRequest,
    ) -> Result<ExecutionResult, &'static str> {
        Err("EXECUTOR_DISABLED")
    }
}

struct JournalAuditSink;

impl AuditSink for JournalAuditSink {
    fn record(&self, event: AuditEvent) {
        eprintln!(
            "JARVIS_AUDIT {}",
            json!({
                "audit_id": event.audit_id,
                "request_id": event.request_id,
                "subject": event.subject,
                "capability": event.capability,
                "target": event.target,
                "outcome": event.outcome,
            })
        );
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(message) = run().await {
        eprintln!("jarvis-core startup failed: {message}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), &'static str> {
    let bind_address = required_bind_address()?;
    let authenticator = load_authenticator()?;
    let listener = bind_private(bind_address)
        .await
        .map_err(|_| "private listener could not be created")?;
    let gateway = CoreGateway::new(PolicyEngine::default(), DisabledExecutor, JournalAuditSink);
    let websocket_origin = required_websocket_origin()?;
    let voice = load_voice_pipeline()?;
    let events = EventBus::default();
    let codex = load_codex_client()?;
    let codex_configured = codex.is_some();
    let conversation = ConversationService::new(voice.clone(), codex, events.clone());
    let transport = Transport::with_config(
        gateway,
        authenticator,
        TransportConfig {
            websocket_origin: Some(websocket_origin),
            ..TransportConfig::default()
        },
    )
    .with_event_bus(events)
    .with_codex_configured(codex_configured)
    .with_voice_pipeline(voice)
    .with_conversation_service(conversation);
    let prometheus_url = env::var("JARVIS_PROMETHEUS_URL")
        .map_err(|_| "JARVIS_PROMETHEUS_URL is required")?
        .parse()
        .map_err(|_| "JARVIS_PROMETHEUS_URL is invalid")?;
    let telemetry_instance = env::var("JARVIS_TELEMETRY_INSTANCE")
        .map_err(|_| "JARVIS_TELEMETRY_INSTANCE is required")?;
    let prometheus = PrometheusTelemetryAdapter::new(prometheus_url, telemetry_instance)
        .map_err(|_| "Prometheus telemetry configuration is invalid")?;
    let prometheus_alerts = Arc::new(prometheus.clone());
    let telemetry = TelemetryService::new(
        vec![Arc::new(prometheus)],
        transport.event_bus(),
        DEFAULT_TELEMETRY_INTERVAL,
    )
    .map_err(|_| "telemetry service configuration is invalid")?;
    let telemetry_task = tokio::spawn(telemetry.run_until(std::future::pending()));
    let prometheus_alert_task = tokio::spawn(run_prometheus_availability_until(
        prometheus_alerts,
        transport.event_bus(),
        std::future::pending(),
    ));
    let wazuh_task = match env::var("JARVIS_WAZUH_RELAY_URL") {
        Ok(value) if !value.trim().is_empty() => {
            let url = value
                .parse()
                .map_err(|_| "JARVIS_WAZUH_RELAY_URL is invalid")?;
            let token = load_secret("wazuh-relay-token", 32)?;
            let poller = WazuhSecurityPoller::new(url, token)
                .map_err(|_| "Wazuh relay configuration is invalid")?;
            eprintln!("jarvis-core Wazuh security poller enabled");
            Some(tokio::spawn(
                poller.run_until(transport.event_bus(), std::future::pending()),
            ))
        }
        _ => {
            eprintln!("jarvis-core Wazuh security poller disabled: JARVIS_WAZUH_RELAY_URL is not configured");
            None
        }
    };

    let mut agent_health_checks = vec![AgentHealthCheck {
        id: "voice",
        label: "VOICE SERVICE",
        url: agent_health_url(
            &env::var("JARVIS_VOICE_URL").map_err(|_| "JARVIS_VOICE_URL is required")?,
            "v1/health",
            "JARVIS_VOICE_URL is invalid",
        )?,
    }];
    if let Some(check) = optional_agent_health_check(
        "JARVIS_MCP_URL",
        "JARVIS_MCP_URL is invalid",
        "mcp",
        "MCP GATEWAY",
        "v1/health",
    )? {
        agent_health_checks.push(check);
    }
    if let Some(check) = optional_agent_health_check(
        "JARVIS_N8N_URL",
        "JARVIS_N8N_URL is invalid",
        "n8n",
        "N8N",
        "healthz",
    )? {
        agent_health_checks.push(check);
    }
    let agent_health_task = {
        let poller = AgentHealthPoller::new(agent_health_checks)
            .map_err(|_| "agent health poller configuration is invalid")?;
        tokio::spawn(poller.run_until(transport.event_bus(), std::future::pending()))
    };

    eprintln!("jarvis-core ready on {bind_address}");
    let result = serve_until(listener, transport, shutdown_signal())
        .await
        .map_err(|_| "network server stopped unexpectedly");
    telemetry_task.abort();
    let _ = telemetry_task.await;
    prometheus_alert_task.abort();
    let _ = prometheus_alert_task.await;
    if let Some(task) = wazuh_task {
        task.abort();
        let _ = task.await;
    }
    agent_health_task.abort();
    let _ = agent_health_task.await;
    result
}

fn agent_health_url(
    base_url: &str,
    health_path: &str,
    invalid_message: &'static str,
) -> Result<Url, &'static str> {
    let base: Url = base_url.parse().map_err(|_| invalid_message)?;
    base.join(health_path).map_err(|_| invalid_message)
}

fn optional_agent_health_check(
    env_var: &'static str,
    invalid_message: &'static str,
    id: &'static str,
    label: &'static str,
    health_path: &str,
) -> Result<Option<AgentHealthCheck>, &'static str> {
    let Some(value) = env::var(env_var)
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        eprintln!("jarvis-core agent health check for {id} disabled: {env_var} is not configured");
        return Ok(None);
    };
    Ok(Some(AgentHealthCheck {
        id,
        label,
        url: agent_health_url(&value, health_path, invalid_message)?,
    }))
}

fn load_codex_client() -> Result<Option<CodexHttpClient>, &'static str> {
    let Some(value) = env::var("JARVIS_CODEX_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };
    let base_url = value.parse().map_err(|_| "JARVIS_CODEX_URL is invalid")?;
    let timeout_seconds = env::var("JARVIS_CODEX_TASK_TIMEOUT_SECONDS")
        .ok()
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| "JARVIS_CODEX_TASK_TIMEOUT_SECONDS is invalid")
        })
        .transpose()?
        .unwrap_or(120);
    let client = CodexHttpClient::new(
        base_url,
        load_secret(CODEX_CREDENTIAL_NAME, 20)?,
        std::time::Duration::from_secs(timeout_seconds),
    )
    .map_err(|_| "Codex service configuration is invalid")?;
    Ok(Some(client))
}

fn load_voice_pipeline() -> Result<VoicePipeline, &'static str> {
    let voice_base_url = env::var("JARVIS_VOICE_URL")
        .map_err(|_| "JARVIS_VOICE_URL is required")?
        .parse()
        .map_err(|_| "JARVIS_VOICE_URL is invalid")?;
    let litellm_base_url = env::var("JARVIS_LITELLM_URL")
        .map_err(|_| "JARVIS_LITELLM_URL is required")?
        .parse()
        .map_err(|_| "JARVIS_LITELLM_URL is invalid")?;
    let model = env::var("JARVIS_LITELLM_MODEL").map_err(|_| "JARVIS_LITELLM_MODEL is required")?;
    VoicePipeline::new(VoicePipelineConfig {
        voice_base_url,
        voice_token: load_secret(VOICE_CREDENTIAL_NAME, 32)?,
        litellm_base_url,
        litellm_token: load_secret(LITELLM_CREDENTIAL_NAME, 20)?,
        model,
    })
    .map_err(|_| "voice pipeline configuration is invalid")
}

fn load_secret(name: &str, minimum_length: usize) -> Result<String, &'static str> {
    let directory = env::var("CREDENTIALS_DIRECTORY")
        .map_err(|_| "systemd credentials directory is required")?;
    let path = Path::new(&directory).join(name);
    let metadata = fs::symlink_metadata(&path).map_err(|_| "service credential is unavailable")?;
    if !metadata.file_type().is_file() || metadata.len() > 4 * 1024 {
        return Err("service credential is not a bounded regular file");
    }
    let secret = fs::read_to_string(path).map_err(|_| "service credential could not be read")?;
    let secret = secret.trim().to_owned();
    if secret.len() < minimum_length {
        return Err("service credential is too short");
    }
    Ok(secret)
}

fn required_websocket_origin() -> Result<String, &'static str> {
    let value = env::var("JARVIS_WEB_ORIGIN").map_err(|_| "JARVIS_WEB_ORIGIN is required")?;
    let uri: http::Uri = value.parse().map_err(|_| "JARVIS_WEB_ORIGIN is invalid")?;
    if uri.scheme_str() != Some("https")
        || uri.authority().is_none()
        || uri.query().is_some()
        || uri.path_and_query().is_some_and(|path| path.path() != "/")
        || value.ends_with('/')
    {
        return Err("JARVIS_WEB_ORIGIN must be an HTTPS origin without a trailing slash");
    }
    Ok(value)
}

fn required_bind_address() -> Result<SocketAddr, &'static str> {
    env::var("JARVIS_CORE_BIND")
        .map_err(|_| "JARVIS_CORE_BIND is required")?
        .parse()
        .map_err(|_| "JARVIS_CORE_BIND is invalid")
}

fn load_authenticator() -> Result<BearerAuthenticator, &'static str> {
    let directory = env::var("CREDENTIALS_DIRECTORY")
        .map_err(|_| "systemd credentials directory is required")?;
    let path = Path::new(&directory).join(CREDENTIAL_NAME);
    let metadata = fs::symlink_metadata(&path).map_err(|_| "credential registry is unavailable")?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_CREDENTIAL_FILE_BYTES {
        return Err("credential registry is not a bounded regular file");
    }
    let bytes = fs::read(path).map_err(|_| "credential registry could not be read")?;
    let registry: CredentialRegistry =
        serde_json::from_slice(&bytes).map_err(|_| "credential registry is invalid")?;
    let records = registry
        .credentials
        .into_iter()
        .map(|entry| CredentialRecord::from_sha256_hex(&entry.sha256, entry.subject, entry.roles))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "credential registry contains an invalid identity")?;
    BearerAuthenticator::new(records).map_err(|_| "credential registry is unusable")
}

async fn shutdown_signal() {
    let interrupt = async {
        tokio::signal::ctrl_c()
            .await
            .expect("interrupt signal handler must install");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("terminate signal handler must install")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = interrupt => {},
        () = terminate => {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_registry_rejects_unknown_fields() {
        let value = br#"{
            "credentials": [{
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "subject": "desktop:test",
                "roles": ["desktop"],
                "token": "must-not-be-accepted"
            }]
        }"#;
        assert!(serde_json::from_slice::<CredentialRegistry>(value).is_err());
    }

    #[test]
    fn credential_registry_accepts_digest_only_records() {
        let value = br#"{
            "credentials": [{
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "subject": "desktop:test",
                "roles": ["desktop"]
            }]
        }"#;
        let registry = serde_json::from_slice::<CredentialRegistry>(value).expect("registry");
        assert_eq!(registry.credentials.len(), 1);
    }

    #[test]
    fn websocket_origin_requires_a_clean_https_origin() {
        std::env::set_var("JARVIS_WEB_ORIGIN", "https://jarvis.example.internal");
        assert_eq!(
            required_websocket_origin().expect("origin"),
            "https://jarvis.example.internal"
        );
        std::env::set_var("JARVIS_WEB_ORIGIN", "http://jarvis.example.internal");
        assert!(required_websocket_origin().is_err());
        std::env::set_var("JARVIS_WEB_ORIGIN", "https://jarvis.example.internal/path");
        assert!(required_websocket_origin().is_err());
        std::env::remove_var("JARVIS_WEB_ORIGIN");
    }
}
