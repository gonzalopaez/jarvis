mod audit;
mod auth;
mod codex;
mod contracts;
#[cfg(feature = "network-server")]
mod conversation;
mod events;
mod executor;
mod gateway;
mod policy;
mod routing;
#[cfg(feature = "network-server")]
mod security;
mod session;
mod telemetry;
mod transport;
mod validation;
#[cfg(feature = "network-server")]
mod voice;

pub use audit::{AuditEvent, AuditSink, MemoryAuditSink};
pub use auth::{
    BearerAuthenticator, CredentialConfigError, CredentialRecord, MAX_BEARER_BYTES,
    MIN_BEARER_BYTES,
};
pub use codex::{
    CodexService, CodexServiceError, CodexTask, CodexTaskRequest, CodexTaskStatus,
    UnavailableCodexService,
};
pub use contracts::{
    ActionRequest, AgentHealthStatus, AgentsResponse, ApiError, AuthContext, ComponentHealth,
    CoreRequest, CoreResponse, JarvisState, OperationalHealth, Principal, ResponseStatus,
    SystemHealth, API_VERSION,
};
#[cfg(feature = "network-server")]
pub use conversation::{CodexClientError, CodexHttpClient, ConversationService};
pub use events::{
    EventBus, EventBusConfigError, EventEnvelope, EventType, DEFAULT_EVENT_CAPACITY,
    MAX_EVENT_CAPACITY,
};
pub use executor::{ExecutionResult, RestrictedExecutor};
pub use gateway::CoreGateway;
pub use policy::{AuthorizationError, Decision, PolicyEngine, Risk, Rule};
pub use routing::{
    AiMode, CapabilityRequest, CapabilityRoute, CapabilityRouter, Complexity,
    DeterministicCapabilityRouter, RequestSource, RoutingDecision,
};
#[cfg(feature = "network-server")]
pub use security::WazuhSecurityPoller;
pub use session::{
    IssuedSession, SessionConfigError, SessionIssueError, SessionStore, DEFAULT_MAX_SESSIONS,
    DEFAULT_SESSION_TTL, MAX_SESSIONS, SESSION_COOKIE_NAME,
};
#[cfg(feature = "network-server")]
pub use telemetry::{run_prometheus_availability_until, PrometheusTelemetryAdapter};
pub use telemetry::{
    OperationalTelemetry, TelemetryAdapter, TelemetryAdapterError, TelemetryService,
    TelemetryServiceConfigError, TelemetrySource, TelemetryValidationError, TemperatureReading,
    UnavailableTelemetryAdapter, DEFAULT_TELEMETRY_ADAPTER_TIMEOUT, DEFAULT_TELEMETRY_INTERVAL,
    MAX_TELEMETRY_INTERVAL, MIN_TELEMETRY_INTERVAL,
};
#[cfg(feature = "network-server")]
pub use transport::{bind_private, serve, serve_until, PrivateListener};
pub use transport::{
    validate_bind_address, AuthError, Authenticator, BindAddressError, Transport, TransportConfig,
    DEFAULT_CONVERSATION_TIMEOUT, DEFAULT_MAX_BODY_BYTES, DEFAULT_REQUEST_TIMEOUT,
};
pub use validation::{validate_request, ValidationError};
#[cfg(feature = "network-server")]
pub use voice::{VoicePipeline, VoicePipelineConfig, VoicePipelineError, VoicePipelineResult};
