use serde::Serialize;
use serde_json::Value;
use std::{collections::VecDeque, sync::{Arc, Mutex}, time::SystemTime};
use tokio::sync::broadcast;

pub const DEFAULT_EVENT_CAPACITY: usize = 256;
pub const MAX_EVENT_CAPACITY: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    SystemSnapshot,
    SystemHeartbeat,
    SystemResyncRequired,
    JarvisStateChanged,
    AgentStatusChanged,
    AgentTaskStarted,
    AgentTaskCompleted,
    AgentTaskFailed,
    TelemetrySnapshot,
    TelemetrySourceStatus,
    SecurityTelemetryUpdated,
    SecurityAlert,
    VoiceSessionStarted,
    VoiceSessionCompleted,
    VoiceSessionFailed,
    RouterDecision,
    CodexTaskCreated,
    CodexTaskStarted,
    CodexTaskAnalyzing,
    CodexToolRequested,
    CodexToolCompleted,
    CodexAuthorizationRequired,
    CodexTaskExecuting,
    CodexTaskCompleted,
    CodexTaskFailed,
    CodexTaskTimeout,
    McpToolStarted,
    McpToolCompleted,
    McpToolFailed,
    AuthorizationRequested,
    AuthorizationApproved,
    AuthorizationDenied,
}

impl EventType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SystemSnapshot => "system.snapshot",
            Self::SystemHeartbeat => "system.heartbeat",
            Self::SystemResyncRequired => "system.resync_required",
            Self::JarvisStateChanged => "jarvis.state.changed",
            Self::AgentStatusChanged => "agent.status.changed",
            Self::AgentTaskStarted => "agent.task.started",
            Self::AgentTaskCompleted => "agent.task.completed",
            Self::AgentTaskFailed => "agent.task.failed",
            Self::TelemetrySnapshot => "telemetry.snapshot",
            Self::TelemetrySourceStatus => "telemetry.source.status",
            Self::SecurityTelemetryUpdated => "security.telemetry.updated",
            Self::SecurityAlert => "security.alert",
            Self::VoiceSessionStarted => "voice.session.started",
            Self::VoiceSessionCompleted => "voice.session.completed",
            Self::VoiceSessionFailed => "voice.session.failed",
            Self::RouterDecision => "router.decision",
            Self::CodexTaskCreated => "codex.task.created",
            Self::CodexTaskStarted => "codex.task.started",
            Self::CodexTaskAnalyzing => "codex.task.analyzing",
            Self::CodexToolRequested => "codex.tool.requested",
            Self::CodexToolCompleted => "codex.tool.completed",
            Self::CodexAuthorizationRequired => "codex.authorization.required",
            Self::CodexTaskExecuting => "codex.task.executing",
            Self::CodexTaskCompleted => "codex.task.completed",
            Self::CodexTaskFailed => "codex.task.failed",
            Self::CodexTaskTimeout => "codex.task.timeout",
            Self::McpToolStarted => "mcp.tool.started",
            Self::McpToolCompleted => "mcp.tool.completed",
            Self::McpToolFailed => "mcp.tool.failed",
            Self::AuthorizationRequested => "authorization.requested",
            Self::AuthorizationApproved => "authorization.approved",
            Self::AuthorizationDenied => "authorization.denied",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EventEnvelope {
    pub event_version: &'static str,
    pub event_id: String,
    #[serde(rename = "type")]
    pub event_type: &'static str,
    pub timestamp_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    pub payload: Value,
}

#[derive(Clone)]
pub struct EventBus {
    inner: Arc<EventBusInner>,
}

struct EventBusInner {
    sender: broadcast::Sender<EventEnvelope>,
    sequence: std::sync::atomic::AtomicU64,
    history: Mutex<VecDeque<EventEnvelope>>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(DEFAULT_EVENT_CAPACITY).expect("default event capacity is valid")
    }
}

impl EventBus {
    pub fn new(capacity: usize) -> Result<Self, EventBusConfigError> {
        if capacity == 0 || capacity > MAX_EVENT_CAPACITY {
            return Err(EventBusConfigError);
        }
        let (sender, _) = broadcast::channel(capacity);
        Ok(Self {
            inner: Arc::new(EventBusInner {
                sender,
                sequence: std::sync::atomic::AtomicU64::new(1),
                history: Mutex::new(VecDeque::with_capacity(64)),
            }),
        })
    }

    pub fn publish(
        &self,
        event_type: EventType,
        correlation_id: Option<String>,
        payload: Value,
    ) -> EventEnvelope {
        let envelope = self.build(event_type, correlation_id, payload);
        if let Ok(mut history) = self.inner.history.lock() {
            history.push_back(envelope.clone());
            while history.len() > 64 { history.pop_front(); }
        }
        let _ = self.inner.sender.send(envelope.clone());
        envelope
    }

    pub fn build(
        &self,
        event_type: EventType,
        correlation_id: Option<String>,
        payload: Value,
    ) -> EventEnvelope {
        let sequence = self
            .inner
            .sequence
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let timestamp_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX);
        EventEnvelope {
            event_version: "v1",
            event_id: format!("event-{sequence:016x}"),
            event_type: event_type.as_str(),
            timestamp_ms,
            correlation_id,
            payload,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.inner.sender.subscribe()
    }

    pub fn recent_security_events(&self) -> Vec<EventEnvelope> {
        self.inner.history.lock().map(|history| history.iter()
            .filter(|event| event.event_type == "security.alert" || event.event_type == "security.telemetry.updated")
            .cloned().collect()).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventBusConfigError;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_unbounded_or_empty_capacity() {
        assert!(EventBus::new(0).is_err());
        assert!(EventBus::new(MAX_EVENT_CAPACITY + 1).is_err());
    }

    #[tokio::test]
    async fn publishes_normalized_versioned_events() {
        let bus = EventBus::new(4).expect("bus");
        let mut receiver = bus.subscribe();
        let published = bus.publish(
            EventType::JarvisStateChanged,
            Some("request-1".into()),
            json!({ "state": "THINKING" }),
        );
        let received = receiver.recv().await.expect("event");
        assert_eq!(published, received);
        assert_eq!(received.event_type, "jarvis.state.changed");
        assert_eq!(received.event_version, "v1");
        assert_eq!(received.correlation_id.as_deref(), Some("request-1"));
    }
}
