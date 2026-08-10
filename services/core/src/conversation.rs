use crate::{
    AiMode, CapabilityRequest, CapabilityRoute, CapabilityRouter, CoreRequest, CoreResponse,
    DeterministicCapabilityRouter, EventBus, EventType, RequestSource, ResponseStatus, API_VERSION,
};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use crate::VoicePipeline;

static CONVERSATION_AUDIT_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const MAX_CODEX_RESPONSE_BYTES: usize = 128 * 1024;

#[derive(Clone)]
pub struct ConversationService {
    router: DeterministicCapabilityRouter,
    models: VoicePipeline,
    codex: Option<CodexHttpClient>,
    events: EventBus,
}

impl ConversationService {
    pub fn new(models: VoicePipeline, codex: Option<CodexHttpClient>, events: EventBus) -> Self {
        Self {
            router: DeterministicCapabilityRouter,
            models,
            codex,
            events,
        }
    }

    pub async fn handle(&self, request: &CoreRequest) -> CoreResponse {
        let message = request.message.as_deref().unwrap_or_default();
        let decision = self.router.decide(&CapabilityRequest {
            message: message.into(),
            session_id: request.session_id.clone(),
            source: RequestSource::Text,
            mode: AiMode::Auto,
        });
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
            CapabilityRoute::InfrastructureAgent => match self.codex_response(request, decision.intent).await {
                Ok(result) => Ok(result),
                Err(_) => Err((
                    "INFRASTRUCTURE_AGENT_UNAVAILABLE",
                    "Infrastructure Agent could not obtain verified data",
                )),
            },
            CapabilityRoute::SecurityAgent => match self.codex_response(request, decision.intent).await {
                Ok(result) => Ok(result),
                Err(_) => Err((
                    "SECURITY_AGENT_UNAVAILABLE",
                    "Security Agent could not obtain verified Wazuh data",
                )),
            },
            CapabilityRoute::Automation => Err((
                "AUTOMATION_UNAVAILABLE",
                "Automation routing is not connected",
            )),
            CapabilityRoute::McpTool => {
                Err(("MCP_TOOL_UNAVAILABLE", "Direct MCP routing is not enabled"))
            }
        };
        match result {
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
        }
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
                Ok((output, "expert"))
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
            CONVERSATION_AUDIT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
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
