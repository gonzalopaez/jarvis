use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CodexTaskStatus {
    Queued,
    Analyzing,
    WaitingTool,
    WaitingAuthorization,
    Executing,
    Completed,
    Failed,
    Cancelled,
    Timeout,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CodexTaskRequest {
    pub task_type: String,
    pub objective: String,
    pub target: Option<String>,
    #[serde(default)]
    pub context: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct CodexTask {
    pub task_id: String,
    pub session_id: String,
    pub correlation_id: String,
    pub status: CodexTaskStatus,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub request: CodexTaskRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexServiceError {
    Unavailable,
    InvalidTask,
    NotFound,
    Timeout,
}

/// The Core depends on this contract, never on CLI output or App Server JSON-RPC.
pub trait CodexService: Send + Sync {
    fn create_task(
        &self,
        session_id: &str,
        correlation_id: &str,
        request: CodexTaskRequest,
    ) -> Result<CodexTask, CodexServiceError>;
    fn continue_task(
        &self,
        task_id: &str,
        request: CodexTaskRequest,
    ) -> Result<CodexTask, CodexServiceError>;
    fn get_task(&self, task_id: &str) -> Result<CodexTask, CodexServiceError>;
    fn cancel_task(&self, task_id: &str) -> Result<CodexTask, CodexServiceError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UnavailableCodexService;

impl CodexService for UnavailableCodexService {
    fn create_task(
        &self,
        _: &str,
        _: &str,
        _: CodexTaskRequest,
    ) -> Result<CodexTask, CodexServiceError> {
        Err(CodexServiceError::Unavailable)
    }
    fn continue_task(&self, _: &str, _: CodexTaskRequest) -> Result<CodexTask, CodexServiceError> {
        Err(CodexServiceError::Unavailable)
    }
    fn get_task(&self, _: &str) -> Result<CodexTask, CodexServiceError> {
        Err(CodexServiceError::Unavailable)
    }
    fn cancel_task(&self, _: &str) -> Result<CodexTask, CodexServiceError> {
        Err(CodexServiceError::Unavailable)
    }
}
