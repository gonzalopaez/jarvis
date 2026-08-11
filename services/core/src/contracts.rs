use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const API_VERSION: &str = "v1";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationalHealth {
    Healthy,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JarvisState {
    Idle,
    Listening,
    Thinking,
    Routing,
    Executing,
    Speaking,
    AuthorizationRequired,
    Warning,
    Error,
    Offline,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentHealthStatus {
    Realtime,
    Ready,
    Busy,
    Degraded,
    Error,
    Offline,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ComponentHealth {
    pub id: &'static str,
    pub label: &'static str,
    pub status: OperationalHealth,
    pub agent_status: AgentHealthStatus,
    pub version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SystemHealth {
    pub api_version: &'static str,
    pub status: OperationalHealth,
    pub state: JarvisState,
    pub components: Vec<ComponentHealth>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AgentsResponse {
    pub api_version: &'static str,
    pub agents: Vec<ComponentHealth>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    pub subject: String,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthContext {
    pub authenticated: bool,
    pub principal: Option<Principal>,
}

impl AuthContext {
    pub fn anonymous() -> Self {
        Self {
            authenticated: false,
            principal: None,
        }
    }

    pub fn authenticated(subject: impl Into<String>, roles: Vec<String>) -> Self {
        Self {
            authenticated: true,
            principal: Some(Principal {
                subject: subject.into(),
                roles,
            }),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CoreRequest {
    pub api_version: String,
    pub request_id: String,
    pub session_id: String,
    pub kind: String,
    pub message: Option<String>,
    pub action: Option<ActionRequest>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ActionRequest {
    pub capability: String,
    pub target: String,
    pub parameters: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Completed,
    AuthorizationRequired,
    Denied,
    Rejected,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CoreResponse {
    pub api_version: &'static str,
    pub request_id: String,
    pub session_id: String,
    pub status: ResponseStatus,
    pub audit_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ApiError {
    pub code: &'static str,
    pub message: &'static str,
}
