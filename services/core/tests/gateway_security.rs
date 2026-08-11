use jarvis_core::{
    ActionRequest, AuthContext, AuthorizationSubmission, CoreGateway, CoreRequest, ExecutionResult,
    MemoryAuditSink, PolicyEngine, ResponseStatus, RestrictedExecutor, API_VERSION,
};
use serde_json::{json, Map, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[derive(Clone)]
struct TestExecutor {
    calls: Arc<AtomicUsize>,
    verified: bool,
}

impl TestExecutor {
    fn verified() -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            verified: true,
        }
    }
}

impl RestrictedExecutor for TestExecutor {
    fn execute(
        &self,
        _principal: &jarvis_core::Principal,
        action: &ActionRequest,
    ) -> Result<ExecutionResult, &'static str> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ExecutionResult {
            verified: self.verified,
            data: json!({ "service": action.target, "status": "ready" }),
        })
    }
}

fn gateway(
    executor: TestExecutor,
) -> (
    CoreGateway<TestExecutor, Arc<MemoryAuditSink>>,
    Arc<MemoryAuditSink>,
) {
    let audit = Arc::new(MemoryAuditSink::default());
    (
        CoreGateway::new(PolicyEngine::default(), executor, Arc::clone(&audit)),
        audit,
    )
}

fn action_request(capability: &str, target: &str) -> CoreRequest {
    CoreRequest {
        api_version: API_VERSION.into(),
        request_id: "req-0001".into(),
        session_id: "session-0001".into(),
        kind: "action".into(),
        message: None,
        action: Some(ActionRequest {
            capability: capability.into(),
            target: target.into(),
            parameters: Map::new(),
        }),
        authorization: None,
    }
}

fn desktop_auth() -> AuthContext {
    AuthContext::authenticated("desktop:test", vec!["desktop".into()])
}

#[test]
fn anonymous_requests_are_denied_before_execution() {
    let executor = TestExecutor::verified();
    let calls = Arc::clone(&executor.calls);
    let (gateway, audit) = gateway(executor);

    let response = gateway.handle(
        &AuthContext::anonymous(),
        action_request("core.health.read", "jarvis-core"),
    );

    assert_eq!(response.status, ResponseStatus::Denied);
    assert_eq!(
        response.error.expect("safe error").code,
        "AUTHENTICATION_REQUIRED"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(audit.events()[0].subject, "anonymous");
}

#[test]
fn unknown_capabilities_are_denied_by_default() {
    let executor = TestExecutor::verified();
    let calls = Arc::clone(&executor.calls);
    let (gateway, _) = gateway(executor);

    let response = gateway.handle(
        &desktop_auth(),
        action_request("shell.execute", "workstation"),
    );

    assert_eq!(response.status, ResponseStatus::Denied);
    assert_eq!(
        response.error.expect("safe error").code,
        "CAPABILITY_DENIED"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn protected_actions_stop_at_authorization_boundary() {
    let executor = TestExecutor::verified();
    let calls = Arc::clone(&executor.calls);
    let (gateway, audit) = gateway(executor);
    let auth = AuthContext::authenticated("operator:test", vec!["operator".into()]);

    let response = gateway.handle(&auth, action_request("security.host.isolate", "host-01"));

    assert_eq!(response.status, ResponseStatus::AuthorizationRequired);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(audit.events()[0].outcome, "authorization_required");
}

#[test]
fn tier_3_http_confirmation_without_rollback_plan_is_rejected() {
    let executor = TestExecutor::verified();
    let calls = Arc::clone(&executor.calls);
    let (gateway, _) = gateway(executor);
    let mut request = action_request("proxmox.vm.destroy", "vm-104");
    request.authorization = Some(AuthorizationSubmission {
        confirmation: "vm-104".into(),
        rollback_plan: None,
    });

    let response = gateway.handle(&desktop_auth(), request);

    assert_eq!(response.status, ResponseStatus::Denied);
    assert_eq!(
        response.error.expect("safe error").code,
        "ROLLBACK_PLAN_REQUIRED"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn tier_3_valid_http_confirmation_reaches_executor_once() {
    let executor = TestExecutor::verified();
    let calls = Arc::clone(&executor.calls);
    let (gateway, _) = gateway(executor);
    let mut request = action_request("proxmox.vm.destroy", "vm-104");
    request.authorization = Some(AuthorizationSubmission {
        confirmation: "vm-104".into(),
        rollback_plan: Some("restore vm-104 from the verified backup".into()),
    });

    let response = gateway.handle(&desktop_auth(), request);

    assert_eq!(response.status, ResponseStatus::Completed);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn domain_agent_cannot_submit_human_confirmation() {
    let executor = TestExecutor::verified();
    let calls = Arc::clone(&executor.calls);
    let (gateway, _) = gateway(executor);
    let mut request = action_request("proxmox.vm.destroy", "vm-104");
    request.authorization = Some(AuthorizationSubmission {
        confirmation: "vm-104".into(),
        rollback_plan: Some("restore vm-104 from the verified backup".into()),
    });
    let agent = AuthContext::authenticated("proxmox-agent:prod", vec!["proxmox-agent".into()]);

    let response = gateway.handle(&agent, request);

    assert_eq!(response.status, ResponseStatus::Denied);
    assert_eq!(
        response.error.expect("safe error").code,
        "ROLE_NOT_AUTHORIZED"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn allowlisted_read_action_requires_verified_result() {
    let executor = TestExecutor::verified();
    let calls = Arc::clone(&executor.calls);
    let (gateway, audit) = gateway(executor);

    let response = gateway.handle(
        &desktop_auth(),
        action_request("core.health.read", "jarvis-core"),
    );

    assert_eq!(response.status, ResponseStatus::Completed);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(audit.events()[0].outcome, "verified");
}

#[test]
fn unverified_executor_result_fails_closed() {
    let executor = TestExecutor {
        calls: Arc::new(AtomicUsize::new(0)),
        verified: false,
    };
    let (gateway, audit) = gateway(executor);

    let response = gateway.handle(
        &desktop_auth(),
        action_request("core.health.read", "jarvis-core"),
    );

    assert_eq!(response.status, ResponseStatus::Denied);
    assert_eq!(
        response.error.expect("safe error").code,
        "EXECUTION_NOT_VERIFIED"
    );
    assert_eq!(audit.events()[0].outcome, "verification_failed");
}

#[test]
fn secret_shaped_action_fields_are_rejected_and_not_audited() {
    let executor = TestExecutor::verified();
    let (gateway, audit) = gateway(executor);
    let mut request = action_request("core.health.read", "jarvis-core");
    request
        .action
        .as_mut()
        .expect("action")
        .parameters
        .insert("api_key".into(), Value::String("not-a-real-value".into()));

    let response = gateway.handle(&desktop_auth(), request);

    assert_eq!(response.status, ResponseStatus::Rejected);
    let event = &audit.events()[0];
    assert_eq!(event.outcome, "rejected");
    assert_eq!(event.capability.as_deref(), Some("core.health.read"));
}

#[test]
fn conversation_path_is_mock_only() {
    let executor = TestExecutor::verified();
    let calls = Arc::clone(&executor.calls);
    let (gateway, _) = gateway(executor);
    let request = CoreRequest {
        api_version: API_VERSION.into(),
        request_id: "req-chat-1".into(),
        session_id: "session-chat-1".into(),
        kind: "conversation".into(),
        message: Some("Report local status.".into()),
        action: None,
        authorization: None,
    };

    let response = gateway.handle(&desktop_auth(), request);

    assert_eq!(response.status, ResponseStatus::Completed);
    assert_eq!(response.data.expect("mock data")["mode"], "mock");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn unknown_json_fields_are_rejected_by_contract() {
    let parsed = serde_json::from_value::<CoreRequest>(json!({
        "api_version": "v1",
        "request_id": "req-1",
        "session_id": "session-1",
        "kind": "conversation",
        "message": "hello",
        "action": null,
        "unexpected": "field"
    }));

    assert!(parsed.is_err());
}
