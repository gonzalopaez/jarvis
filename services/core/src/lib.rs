mod audit;
mod contracts;
mod executor;
mod gateway;
mod policy;
mod transport;
mod validation;

pub use audit::{AuditEvent, AuditSink, MemoryAuditSink};
pub use contracts::{
    ActionRequest, ApiError, AuthContext, CoreRequest, CoreResponse, Principal, ResponseStatus,
    API_VERSION,
};
pub use executor::{ExecutionResult, RestrictedExecutor};
pub use gateway::CoreGateway;
pub use policy::{Decision, PolicyEngine, Risk, Rule};
#[cfg(feature = "network-server")]
pub use transport::serve;
pub use transport::{
    AuthError, Authenticator, Transport, TransportConfig, DEFAULT_MAX_BODY_BYTES,
    DEFAULT_REQUEST_TIMEOUT,
};
pub use validation::{validate_request, ValidationError};
