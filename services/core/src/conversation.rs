use crate::{
    AiMode, CapabilityRequest, CapabilityRoute, CapabilityRouter, CoreRequest, CoreResponse,
    DeterministicCapabilityRouter, EventBus, EventType, RequestSource, ResponseStatus,
    RoutingDecision, API_VERSION,
};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    future::Future,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use crate::auth::OneTimeGrantStore;
use crate::{AvailabilityProvider, AvailabilityTarget, VoicePipeline};
use std::sync::Arc;

static CONVERSATION_AUDIT_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const MAX_CODEX_RESPONSE_BYTES: usize = 128 * 1024;
const MAX_PENDING_MITIGATIONS: usize = 128;
const PENDING_MITIGATION_TTL: Duration = Duration::from_secs(5 * 60);

async fn fan_out_evidence<A, B>(infrastructure: A, security: B) -> (A::Output, B::Output)
where
    A: Future,
    B: Future,
{
    tokio::join!(infrastructure, security)
}

fn is_affirmative(message: &str) -> bool {
    matches!(
        message.trim().to_lowercase().as_str(),
        "si" | "sí" | "sí." | "si."
    )
}

fn security_remediation_decision() -> RoutingDecision {
    RoutingDecision {
        route: CapabilityRoute::Codex,
        intent: "security_remediation",
        complexity: crate::Complexity::High,
        agent: Some("codex"),
        model_alias: None,
        requires_tools: true,
        requires_authorization: true,
        reason: "operator confirmed security mitigation handoff",
    }
}

#[derive(Clone)]
pub struct ConversationService {
    router: DeterministicCapabilityRouter,
    models: VoicePipeline,
    codex: Option<CodexHttpClient>,
    events: EventBus,
    pending_mitigation: OneTimeGrantStore<String>,
    availability: Option<Arc<dyn AvailabilityProvider>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ConversationTimings {
    pub routing_ms: u64,
    pub llm_ms: u64,
}

impl ConversationService {
    pub fn new(
        models: VoicePipeline,
        codex: Option<CodexHttpClient>,
        events: EventBus,
        availability: Option<Arc<dyn AvailabilityProvider>>,
    ) -> Self {
        Self {
            router: DeterministicCapabilityRouter,
            models,
            codex,
            events,
            pending_mitigation: OneTimeGrantStore::new(MAX_PENDING_MITIGATIONS),
            availability,
        }
    }

    pub async fn handle(&self, request: &CoreRequest) -> CoreResponse {
        self.handle_with_timings(request).await.0
    }

    pub(crate) async fn handle_with_timings(
        &self,
        request: &CoreRequest,
    ) -> (CoreResponse, ConversationTimings) {
        let message = request.message.as_deref().unwrap_or_default();
        let routing_started = Instant::now();
        let mut decision = self.router.decide(&CapabilityRequest {
            message: message.into(),
            session_id: request.session_id.clone(),
            source: RequestSource::Text,
            mode: AiMode::Auto,
        });
        if is_affirmative(message) && self.take_pending_mitigation(&request.session_id) {
            decision = security_remediation_decision();
        }
        let routing_ms = elapsed_ms(routing_started);
        let correlation = Some(request.request_id.clone());
        self.events.publish(EventType::RouterDecision, correlation.clone(), json!({
            "route": decision.route, "intent": decision.intent, "complexity": decision.complexity,
            "agent": decision.agent, "model_alias": decision.model_alias,
            "requires_tools": decision.requires_tools, "requires_authorization": decision.requires_authorization,
            "reason": decision.reason,
        }));
        self.events.publish(
            EventType::JarvisStateChanged,
            correlation.clone(),
            json!({ "previous": "THINKING", "current": "ROUTING", "state": "ROUTING" }),
        );

        let llm_started = Instant::now();
        let result = match decision.route {
            CapabilityRoute::FastModel => {
                self.model_response(
                    message,
                    decision.model_alias.unwrap_or("jarvis-fast"),
                    "fast",
                )
                .await
            }
            CapabilityRoute::ReasoningModel => {
                self.model_response(
                    message,
                    decision.model_alias.unwrap_or("jarvis-reasoning"),
                    "smart",
                )
                .await
            }
            CapabilityRoute::Codex => match self.codex_response(request, decision.intent).await {
                Ok(result) => Ok(result),
                Err(_) => {
                    self.events.publish(EventType::RouterDecision, correlation.clone(), json!({ "route": "REASONING_MODEL", "model_alias": "jarvis-reasoning", "reason": "Codex unavailable; safe reasoning fallback" }));
                    self.model_response(message, "jarvis-reasoning", "fallback")
                        .await
                        .map_err(|_| {
                            (
                                "CODEX_UNAVAILABLE",
                                "Codex Agent unavailable and reasoning fallback failed",
                            )
                        })
                }
            },
            CapabilityRoute::InfrastructureAgent => {
                if decision.intent == "service_availability" {
                    self.availability_response(message).await
                } else {
                    match self.codex_response(request, decision.intent).await {
                        Ok(result) => Ok(result),
                        Err(_) => Err((
                            "INFRASTRUCTURE_AGENT_UNAVAILABLE",
                            "Infrastructure Agent could not obtain verified data",
                        )),
                    }
                }
            }
            CapabilityRoute::CrossDomainAgents => self.cross_domain_response(request).await,
            CapabilityRoute::SecurityAgent => self.security_response(request),
            CapabilityRoute::Automation => Err((
                "AUTOMATION_UNAVAILABLE",
                "Automation routing is not connected",
            )),
            CapabilityRoute::McpTool => {
                Err(("MCP_TOOL_UNAVAILABLE", "Direct MCP routing is not enabled"))
            }
        };
        let llm_ms = elapsed_ms(llm_started);
        let response = match result {
            Ok((message, mode)) => response(
                request,
                ResponseStatus::Completed,
                Some(json!({ "message": message, "mode": mode, "route": decision.route })),
                None,
            ),
            Err((code, safe_message)) => {
                self.events.publish(
                    EventType::JarvisStateChanged,
                    correlation,
                    json!({ "current": "ERROR", "state": "ERROR" }),
                );
                response(
                    request,
                    ResponseStatus::Rejected,
                    None,
                    Some(crate::ApiError {
                        code,
                        message: safe_message,
                    }),
                )
            }
        };
        (response, ConversationTimings { routing_ms, llm_ms })
    }

    async fn availability_response(
        &self,
        message: &str,
    ) -> Result<(String, &'static str), (&'static str, &'static str)> {
        let provider = self.availability.as_ref().ok_or((
            "TELEMETRY_UNAVAILABLE",
            "No pude verificar el estado actual en Prometheus.",
        ))?;
        let targets = provider.current_availability().await.map_err(|_| {
            (
                "TELEMETRY_UNAVAILABLE",
                "No pude verificar el estado actual en Prometheus.",
            )
        })?;
        let normalized = normalize_availability_query(message);
        let selection = availability_selection(&normalized);
        let selected = targets
            .iter()
            .filter(|target| selection.matches(target))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Ok((
                "No encontré ese componente en el inventario verificado de Prometheus.".into(),
                "infrastructure",
            ));
        }
        if selection == AvailabilitySelection::AllDown {
            let down = selected
                .into_iter()
                .filter(|target| !target.up)
                .collect::<Vec<_>>();
            if down.is_empty() {
                return Ok((
                    "Prometheus no reporta servicios o equipos caídos en este momento.".into(),
                    "infrastructure",
                ));
            }
            let names = down
                .iter()
                .map(|target| availability_label(target))
                .collect::<Vec<_>>()
                .join(", ");
            return Ok((
                format!("Prometheus reporta caídos: {names}."),
                "infrastructure",
            ));
        }
        let up = selected.iter().all(|target| target.up);
        let label = selection.label();
        let detail = selected
            .iter()
            .map(|target| availability_label(target))
            .collect::<Vec<_>>()
            .join(", ");
        Ok((
            format!(
                "{label} está {} según Prometheus ({detail}).",
                if up { "online" } else { "caído o degradado" }
            ),
            "infrastructure",
        ))
    }

    async fn cross_domain_response(
        &self,
        request: &CoreRequest,
    ) -> Result<(String, &'static str), (&'static str, &'static str)> {
        let infrastructure = self.codex_response(request, "infrastructure_diagnostic");
        let security = async { self.security_response(request) };
        let (infrastructure, security) = fan_out_evidence(infrastructure, security).await;

        match (infrastructure, security) {
            (Ok((infrastructure, _)), Ok((security, _))) => Ok((
                format!(
                    "Evidencia de infraestructura: {infrastructure}\nEvidencia de seguridad: {security}"
                ),
                "multi_agent",
            )),
            (Err(error), _) => Err(error),
            (_, Err(error)) => Err(error),
        }
    }

    fn security_response(
        &self,
        request: &CoreRequest,
    ) -> Result<(String, &'static str), (&'static str, &'static str)> {
        let availability_text = request
            .message
            .as_deref()
            .unwrap_or_default()
            .to_lowercase()
            .chars()
            .map(|character| if character == 'í' { 'i' } else { character })
            .collect::<String>();
        let critical_only = availability_text.contains("critic");
        let availability_only = availability_text.contains("caido")
            || availability_text.contains("caida")
            || availability_text.contains("servicio")
            || availability_text.contains("servidor");
        let target_query = availability_text;
        let target = if target_query.contains("vpn") || target_query.contains("uve pene") {
            Some("vpn")
        } else if target_query.contains("cloudflare") || target_query.contains("tunnel") {
            Some("tunnel")
        } else if target_query.contains("dc") || target_query.contains("de ce") {
            Some("dc")
        } else if target_query.contains("freeipa") {
            Some("freeipa")
        } else if target_query.contains("adguard") {
            Some("adguard")
        } else if target_query.contains("tailscale") {
            Some("tailscale")
        } else {
            None
        };
        let mut seen = HashSet::new();
        let mut alerts = self
            .events
            .recent_security_events()
            .into_iter()
            .filter(|event| event.event_type == "security.alert")
            .filter_map(|event| {
                let id = event.payload.get("id")?.as_str()?.to_owned();
                if !seen.insert(id.clone()) {
                    return None;
                }
                let severity = event.payload.get("severity")?.as_str()?.to_lowercase();
                if critical_only && severity != "critical" {
                    return None;
                }
                let title = event.payload.get("title")?.as_str()?.to_owned();
                let description = event
                    .payload
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let host = event
                    .payload
                    .get("host")
                    .and_then(Value::as_str)
                    .unwrap_or("host desconocido")
                    .to_owned();
                if availability_only
                    && !title.to_lowercase().contains("caído")
                    && !title.to_lowercase().contains("caido")
                    && !description
                        .to_lowercase()
                        .contains("prometheus reporta down")
                {
                    return None;
                }
                if let Some(target) = target {
                    let host = host.to_lowercase();
                    let matches = match target {
                        "vpn" => {
                            host.contains("vpn")
                                || host.contains("tunnel")
                                || host.contains("tailscale")
                        }
                        "tunnel" => host.contains("tunnel") || host.contains("cloudflare"),
                        value => host.contains(value),
                    };
                    if !matches {
                        return None;
                    }
                }
                Some((severity, host, title))
            })
            .collect::<Vec<_>>();
        alerts.reverse();
        let window = if availability_only {
            "de disponibilidad"
        } else if critical_only {
            "críticas"
        } else {
            "recientes"
        };
        if alerts.is_empty() {
            return Ok((
                format!("No hay alertas Wazuh {window} en la ventana de eventos disponible.",),
                "security",
            ));
        }
        let mut answer = format!("Hay {} alertas Wazuh {window}. ", alerts.len());
        for (index, (severity, host, title)) in alerts.drain(..).take(5).enumerate() {
            if index > 0 {
                answer.push(' ');
            }
            answer.push_str(&format!("{}: {} en {}.", severity, title, host));
        }
        if !availability_only {
            let _ = self
                .pending_mitigation
                .issue_at(request.session_id.clone(), Instant::now());
            answer
                .push_str(" ¿Querés que evaluemos cómo mitigar el riesgo aumentando la seguridad?");
        }
        Ok((answer, "security"))
    }

    fn take_pending_mitigation(&self, session_id: &str) -> bool {
        self.pending_mitigation.take_at(
            &session_id.to_string(),
            PENDING_MITIGATION_TTL,
            Instant::now(),
        )
    }

    async fn model_response(
        &self,
        message: &str,
        alias: &str,
        mode: &'static str,
    ) -> Result<(String, &'static str), (&'static str, &'static str)> {
        self.events.publish(
            EventType::JarvisStateChanged,
            None,
            json!({ "current": "THINKING", "state": "THINKING" }),
        );
        self.models
            .complete_text(message, alias)
            .await
            .map(|output| (output, mode))
            .map_err(|_| ("MODEL_UNAVAILABLE", "Configured model is unavailable"))
    }

    async fn codex_response(
        &self,
        request: &CoreRequest,
        intent: &str,
    ) -> Result<(String, &'static str), (&'static str, &'static str)> {
        let correlation = Some(request.request_id.clone());
        let Some(codex) = &self.codex else {
            self.events.publish(
                EventType::AgentStatusChanged,
                correlation,
                codex_agent("OFFLINE", "unavailable", Some("not_connected")),
            );
            return Err(("CODEX_UNAVAILABLE", "Codex Agent is not configured"));
        };
        self.events.publish(
            EventType::CodexTaskCreated,
            correlation.clone(),
            json!({ "session_id": request.session_id, "status": "QUEUED" }),
        );
        self.events.publish(
            EventType::AgentStatusChanged,
            correlation.clone(),
            codex_agent("BUSY", "healthy", None),
        );
        self.events.publish(
            EventType::CodexTaskAnalyzing,
            correlation.clone(),
            json!({ "session_id": request.session_id, "status": "ANALYZING" }),
        );
        self.events.publish(
            EventType::JarvisStateChanged,
            correlation.clone(),
            json!({ "current": "THINKING", "state": "THINKING", "context": "CODEX // ANALYZING" }),
        );
        let result = codex
            .execute(
                &request.session_id,
                &request.request_id,
                intent,
                request.message.as_deref().unwrap_or_default(),
            )
            .await;
        match result {
            Ok((task_id, output)) => {
                self.events.publish(
                    EventType::CodexTaskCompleted,
                    correlation.clone(),
                    json!({ "task_id": task_id, "status": "COMPLETED" }),
                );
                self.events.publish(
                    EventType::AgentStatusChanged,
                    correlation,
                    codex_agent("READY", "healthy", None),
                );
                if intent == "security_remediation" {
                    Ok((
                        format!("Ya se lo pasé a Codex para evaluar la remediación de seguridad. {output}"),
                        "expert",
                    ))
                } else {
                    Ok((output, "expert"))
                }
            }
            Err(CodexClientError::Timeout) => {
                self.events.publish(
                    EventType::CodexTaskTimeout,
                    correlation.clone(),
                    json!({ "status": "TIMEOUT" }),
                );
                self.events.publish(
                    EventType::AgentStatusChanged,
                    correlation,
                    codex_agent("DEGRADED", "degraded", Some("timeout")),
                );
                Err(("CODEX_TIMEOUT", "Codex Agent task timed out"))
            }
            Err(_) => {
                self.events.publish(
                    EventType::CodexTaskFailed,
                    correlation.clone(),
                    json!({ "status": "FAILED" }),
                );
                self.events.publish(
                    EventType::AgentStatusChanged,
                    correlation,
                    codex_agent("DEGRADED", "degraded", Some("unavailable")),
                );
                Err(("CODEX_UNAVAILABLE", "Codex Agent unavailable"))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AvailabilitySelection {
    AllDown,
    Vpn,
    Cloudflare,
    Firewall,
}

impl AvailabilitySelection {
    fn matches(self, target: &AvailabilityTarget) -> bool {
        let name = target.name.to_lowercase();
        let service = target.service.as_deref().unwrap_or_default().to_lowercase();
        match self {
            Self::AllDown => true,
            Self::Vpn => {
                name.contains("tailscale") || name.contains("vpn") || service.contains("tailscale")
            }
            Self::Cloudflare => {
                name.contains("cloudflare")
                    || name.contains("tunnel")
                    || service.contains("cloudflared")
            }
            Self::Firewall => {
                name.contains("opnsense") || name.contains("pfsense") || name.contains("firewall")
            }
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::AllDown => "La infraestructura",
            Self::Vpn => "La VPN",
            Self::Cloudflare => "El túnel de Cloudflare",
            Self::Firewall => "El firewall OPNsense/pfSense",
        }
    }
}

fn availability_selection(normalized: &str) -> AvailabilitySelection {
    if normalized.contains("cloudflare") || normalized.contains("tunnel") {
        AvailabilitySelection::Cloudflare
    } else if normalized.contains("pfsense")
        || normalized.contains("psfesense")
        || normalized.contains("opnsense")
        || normalized.contains("firewall")
    {
        AvailabilitySelection::Firewall
    } else if normalized.contains("vpn")
        || normalized.contains("tailscale")
        || normalized.contains("uve pene")
    {
        AvailabilitySelection::Vpn
    } else {
        AvailabilitySelection::AllDown
    }
}

fn normalize_availability_query(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .map(|character| match character {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' | 'ü' => 'u',
            other => other,
        })
        .collect()
}

fn availability_label(target: &AvailabilityTarget) -> String {
    match &target.service {
        Some(service) => format!("{} / {} (VMID {})", target.name, service, target.vmid),
        None => format!("{} (VMID {})", target.name, target.vmid),
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

fn codex_agent(
    agent_status: &'static str,
    status: &'static str,
    error: Option<&'static str>,
) -> Value {
    json!({ "id": "codex", "label": "CODEX AGENT", "status": status, "agent_status": agent_status, "version": "sdk", "error": error })
}

fn response(
    request: &CoreRequest,
    status: ResponseStatus,
    data: Option<Value>,
    error: Option<crate::ApiError>,
) -> CoreResponse {
    CoreResponse {
        api_version: API_VERSION,
        request_id: request.request_id.clone(),
        session_id: request.session_id.clone(),
        status,
        audit_id: format!(
            "conversation-{:016x}",
            CONVERSATION_AUDIT_SEQUENCE.fetch_add(1, Ordering::SeqCst)
        ),
        data,
        error,
    }
}

#[derive(Clone)]
pub struct CodexHttpClient {
    client: Client,
    base_url: Url,
    token: String,
    task_timeout: Duration,
    poll_interval: Duration,
}

impl CodexHttpClient {
    pub fn new(
        base_url: Url,
        token: String,
        task_timeout: Duration,
    ) -> Result<Self, CodexClientError> {
        if base_url.scheme() != "http"
            || base_url.host_str().is_none()
            || token.len() < 20
            || !(Duration::from_secs(10)..=Duration::from_secs(600)).contains(&task_timeout)
        {
            return Err(CodexClientError::InvalidConfiguration);
        }
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| CodexClientError::InvalidConfiguration)?;
        Ok(Self {
            client,
            base_url,
            token,
            task_timeout,
            poll_interval: Duration::from_millis(500),
        })
    }

    async fn execute(
        &self,
        session_id: &str,
        correlation_id: &str,
        task_type: &str,
        objective: &str,
    ) -> Result<(String, String), CodexClientError> {
        let url = self
            .base_url
            .join("v1/tasks")
            .map_err(|_| CodexClientError::InvalidConfiguration)?;
        let response = self.client.post(url).bearer_auth(&self.token).json(&json!({ "task_type": task_type, "objective": objective, "session_id": session_id, "correlation_id": correlation_id, "context": { "available_capabilities": [] } })).send().await.map_err(|_| CodexClientError::Unavailable)?;
        if response.status().as_u16() != 202 {
            return Err(CodexClientError::Unavailable);
        }
        let created: RemoteTask = bounded_json(response).await?;
        let started = Instant::now();
        loop {
            if started.elapsed() >= self.task_timeout {
                return Err(CodexClientError::Timeout);
            }
            tokio::time::sleep(self.poll_interval).await;
            let task_url = self
                .base_url
                .join(&format!("v1/tasks/{}", created.task_id))
                .map_err(|_| CodexClientError::InvalidConfiguration)?;
            let response = self
                .client
                .get(task_url)
                .bearer_auth(&self.token)
                .send()
                .await
                .map_err(|_| CodexClientError::Unavailable)?;
            if !response.status().is_success() {
                return Err(CodexClientError::Unavailable);
            }
            let task: RemoteTask = bounded_json(response).await?;
            match task.status.as_str() {
                "COMPLETED" => {
                    return task
                        .result
                        .map(|result| (task.task_id, result.output))
                        .ok_or(CodexClientError::InvalidResponse)
                }
                "FAILED" | "CANCELLED" => return Err(CodexClientError::Unavailable),
                "TIMEOUT" => return Err(CodexClientError::Timeout),
                "QUEUED" | "ANALYZING" | "EXECUTING" | "WAITING_TOOL" | "WAITING_AUTHORIZATION" => {
                }
                _ => return Err(CodexClientError::InvalidResponse),
            }
        }
    }
}

#[derive(Deserialize)]
struct RemoteTask {
    task_id: String,
    status: String,
    result: Option<RemoteResult>,
}
#[derive(Deserialize)]
struct RemoteResult {
    output: String,
}

async fn bounded_json<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
) -> Result<T, CodexClientError> {
    let bytes = response
        .bytes()
        .await
        .map_err(|_| CodexClientError::InvalidResponse)?;
    if bytes.len() > MAX_CODEX_RESPONSE_BYTES {
        return Err(CodexClientError::InvalidResponse);
    }
    serde_json::from_slice(&bytes).map_err(|_| CodexClientError::InvalidResponse)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexClientError {
    InvalidConfiguration,
    Unavailable,
    InvalidResponse,
    Timeout,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VoicePipelineConfig;
    use std::pin::Pin;
    use std::sync::Arc;
    use tokio::sync::Barrier;

    struct FixedAvailability(Vec<AvailabilityTarget>);

    impl AvailabilityProvider for FixedAvailability {
        fn current_availability(
            &self,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<Vec<AvailabilityTarget>, crate::TelemetryAdapterError>>
                    + Send
                    + '_,
            >,
        > {
            Box::pin(async { Ok(self.0.clone()) })
        }
    }

    fn service(events: EventBus) -> ConversationService {
        let voice = VoicePipeline::new(VoicePipelineConfig {
            voice_base_url: "http://voice.internal/".parse().expect("voice URL"),
            voice_token: "v".repeat(32),
            litellm_base_url: "http://litellm.internal/".parse().expect("LiteLLM URL"),
            litellm_token: "l".repeat(20),
            model: "jarvis-fast".into(),
        })
        .expect("voice configuration");
        ConversationService::new(voice, None, events, None)
    }

    fn request(session_id: &str, message: &str) -> CoreRequest {
        CoreRequest {
            api_version: API_VERSION.into(),
            request_id: format!("request-{session_id}"),
            session_id: session_id.into(),
            kind: "conversation".into(),
            message: Some(message.into()),
            action: None,
            authorization: None,
        }
    }

    fn service_with_availability(targets: Vec<AvailabilityTarget>) -> ConversationService {
        let mut service = service(EventBus::default());
        service.availability = Some(Arc::new(FixedAvailability(targets)));
        service
    }

    #[tokio::test]
    async fn availability_answers_use_current_verified_state() {
        let service = service_with_availability(vec![
            AvailabilityTarget {
                name: "tailscale-vpn".into(),
                service: None,
                vmid: "109".into(),
                up: true,
            },
            AvailabilityTarget {
                name: "tailscale-vpn".into(),
                service: Some("tailscaled".into()),
                vmid: "109".into(),
                up: true,
            },
            AvailabilityTarget {
                name: "cloudflare-tunnel".into(),
                service: Some("cloudflared".into()),
                vmid: "105".into(),
                up: false,
            },
            AvailabilityTarget {
                name: "opnsense".into(),
                service: None,
                vmid: "102".into(),
                up: true,
            },
        ]);

        let (vpn, _) = service
            .availability_response("¿La VPN está online?")
            .await
            .expect("VPN response");
        let (tunnel, _) = service
            .availability_response("¿El túnel de Cloudflare está activo?")
            .await
            .expect("tunnel response");
        let (firewall, _) = service
            .availability_response("¿El firewall psfesense está ok?")
            .await
            .expect("firewall response");

        assert!(vpn.contains("VPN está online"));
        assert!(tunnel.contains("caído o degradado"));
        assert!(firewall.contains("OPNsense/pfSense está online"));
    }

    #[tokio::test]
    async fn down_service_list_does_not_depend_on_event_history() {
        let service = service_with_availability(vec![
            AvailabilityTarget {
                name: "freeipa".into(),
                service: None,
                vmid: "108".into(),
                up: false,
            },
            AvailabilityTarget {
                name: "adguard".into(),
                service: None,
                vmid: "101".into(),
                up: true,
            },
        ]);

        let (answer, _) = service
            .availability_response("¿Qué servicios están caídos?")
            .await
            .expect("availability response");

        assert!(answer.contains("freeipa (VMID 108)"));
        assert!(!answer.contains("adguard"));
    }

    #[test]
    fn security_answers_use_recent_deduplicated_alerts() {
        let events = EventBus::default();
        for _ in 0..2 {
            events.publish(
                EventType::SecurityAlert,
                None,
                json!({
                    "id": "wazuh-42",
                    "host": "vpn-01",
                    "severity": "critical",
                    "title": "Servicio caído",
                    "description": "Prometheus reporta down"
                }),
            );
        }
        let service = service(events);

        let (answer, mode) = service
            .security_response(&request("session-a", "¿Está caído el servidor VPN?"))
            .expect("security response");

        assert_eq!(mode, "security");
        assert!(answer.contains("Hay 1 alertas Wazuh de disponibilidad"));
        assert!(answer.contains("vpn-01"));
    }

    #[test]
    fn mitigation_confirmation_is_strictly_session_scoped_and_one_time() {
        let events = EventBus::default();
        events.publish(
            EventType::SecurityAlert,
            None,
            json!({
                "id": "wazuh-43",
                "host": "dc-01",
                "severity": "critical",
                "title": "Intentos de acceso",
                "description": "Múltiples accesos fallidos"
            }),
        );
        let service = service(events);
        service
            .security_response(&request("session-a", "Mostrame las alertas críticas"))
            .expect("security response");

        assert!(!service.take_pending_mitigation("session-b"));
        assert!(service.take_pending_mitigation("session-a"));
        assert!(!service.take_pending_mitigation("session-a"));
    }

    #[test]
    fn expired_mitigation_confirmation_is_rejected() {
        let service = service(EventBus::default());
        service.pending_mitigation.issue_at(
            "session-a".into(),
            Instant::now() - PENDING_MITIGATION_TTL - Duration::from_secs(1),
        );

        assert!(!service.take_pending_mitigation("session-a"));
    }

    #[tokio::test]
    async fn cross_domain_evidence_is_requested_concurrently() {
        let started = Arc::new(Barrier::new(3));
        let first = {
            let started = Arc::clone(&started);
            async move {
                started.wait().await;
                "infrastructure"
            }
        };
        let second = {
            let started = Arc::clone(&started);
            async move {
                started.wait().await;
                "security"
            }
        };

        let joined = fan_out_evidence(first, second);
        let release = async {
            started.wait().await;
        };
        let ((infrastructure, security), ()) = tokio::join!(joined, release);

        assert_eq!(infrastructure, "infrastructure");
        assert_eq!(security, "security");
    }

    #[test]
    fn audit_ids_remain_unique_during_concurrent_fan_out() {
        let request = request("session-a", "Correlacioná infraestructura y seguridad");
        let responses = std::thread::scope(|scope| {
            let handles = (0..32)
                .map(|_| {
                    scope.spawn(|| {
                        response(
                            &request,
                            ResponseStatus::Completed,
                            Some(json!({ "message": "bounded evidence" })),
                            None,
                        )
                        .audit_id
                    })
                })
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("audit response"))
                .collect::<Vec<_>>()
        });
        let unique = responses.iter().collect::<HashSet<_>>();

        assert_eq!(responses.len(), unique.len());
        assert!(responses
            .iter()
            .all(|audit_id| audit_id.starts_with("conversation-") && audit_id.len() == 29));
    }
}
