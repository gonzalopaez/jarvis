use bytes::Bytes;
use http::{header, Method, Request, StatusCode};
use http_body::{Body, Frame, SizeHint};
use http_body_util::{BodyExt, Full};
use jarvis_core::{
    ActionRequest, AuthContext, AuthError, Authenticator, CoreGateway, ExecutionResult,
    MemoryAuditSink, PolicyEngine, RestrictedExecutor, Transport, TransportConfig, API_VERSION,
};
use serde_json::{json, Value};
use std::{
    convert::Infallible,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Poll},
    time::Duration,
};

#[derive(Clone)]
struct CountingExecutor {
    calls: Arc<AtomicUsize>,
}

impl CountingExecutor {
    fn new() -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl RestrictedExecutor for CountingExecutor {
    fn execute(
        &self,
        _principal: &jarvis_core::Principal,
        action: &ActionRequest,
    ) -> Result<ExecutionResult, &'static str> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ExecutionResult {
            verified: true,
            data: json!({ "target": action.target, "status": "ready" }),
        })
    }
}

struct TestAuthenticator;

impl Authenticator for TestAuthenticator {
    fn authenticate(&self, credential: Option<&str>) -> Result<AuthContext, AuthError> {
        match credential {
            Some("Test desktop") => Ok(AuthContext::authenticated(
                "desktop:test",
                vec!["desktop".into()],
            )),
            Some("Test operator") => Ok(AuthContext::authenticated(
                "operator:test",
                vec!["operator".into()],
            )),
            _ => Err(AuthError),
        }
    }
}

fn transport(
    executor: CountingExecutor,
) -> Transport<CountingExecutor, Arc<MemoryAuditSink>, TestAuthenticator> {
    let gateway = CoreGateway::new(
        PolicyEngine::default(),
        executor,
        Arc::new(MemoryAuditSink::default()),
    );
    Transport::new(gateway, TestAuthenticator)
}

fn conversation() -> Value {
    json!({
        "api_version": API_VERSION,
        "request_id": "req-transport-1",
        "session_id": "session-transport-1",
        "kind": "conversation",
        "message": "Report status."
    })
}

fn action(capability: &str, target: &str) -> Value {
    json!({
        "api_version": API_VERSION,
        "request_id": "req-action-1",
        "session_id": "session-action-1",
        "kind": "action",
        "action": {
            "capability": capability,
            "target": target,
            "parameters": {}
        }
    })
}

fn request(body: Value, credential: Option<&str>) -> Request<Full<Bytes>> {
    let mut builder = Request::builder()
        .method(Method::POST)
        .uri("/v1/requests")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(credential) = credential {
        builder = builder.header(header::AUTHORIZATION, credential);
    }
    builder
        .body(Full::new(Bytes::from(
            serde_json::to_vec(&body).expect("fixture serializes"),
        )))
        .expect("request fixture is valid")
}

async fn body_json(response: http::Response<Full<Bytes>>) -> Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("response body")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("response is JSON")
}

#[tokio::test]
async fn health_is_minimal_and_does_not_require_credentials() {
    let response = transport(CountingExecutor::new())
        .handle(
            Request::builder()
                .uri("/v1/health")
                .body(Full::new(Bytes::new()))
                .expect("request"),
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body = body_json(response).await;
    assert_eq!(body["status"], "ready");
    assert_eq!(body.as_object().expect("object").len(), 2);
}

#[tokio::test]
async fn anonymous_core_requests_are_rejected_before_body_processing() {
    let executor = CountingExecutor::new();
    let calls = Arc::clone(&executor.calls);
    let response = transport(executor)
        .handle(request(conversation(), None))
        .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn authenticated_conversation_uses_mock_path() {
    let response = transport(CountingExecutor::new())
        .handle(request(conversation(), Some("Test desktop")))
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["data"]["mode"], "mock");
}

#[tokio::test]
async fn malformed_or_unknown_json_fields_are_rejected() {
    let mut body = conversation();
    body["roles"] = json!(["administrator"]);
    let response = transport(CountingExecutor::new())
        .handle(request(body, Some("Test desktop")))
        .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "INVALID_JSON");
}

#[tokio::test]
async fn unsupported_content_type_is_rejected() {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/v1/requests")
        .header(header::AUTHORIZATION, "Test desktop")
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Full::new(Bytes::from_static(b"hello")))
        .expect("request");
    let response = transport(CountingExecutor::new()).handle(request).await;

    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn oversized_body_is_rejected() {
    let executor = CountingExecutor::new();
    let gateway = CoreGateway::new(
        PolicyEngine::default(),
        executor,
        Arc::new(MemoryAuditSink::default()),
    );
    let transport = Transport::with_config(
        gateway,
        TestAuthenticator,
        TransportConfig {
            max_body_bytes: 32,
            request_timeout: Duration::from_secs(1),
        },
    );
    let request = Request::builder()
        .method(Method::POST)
        .uri("/v1/requests")
        .header(header::AUTHORIZATION, "Test desktop")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from_static(
            b"{\"message\":\"this body is deliberately larger than thirty two bytes\"}",
        )))
        .expect("request");

    let response = transport.handle(request).await;

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn denied_action_never_reaches_executor() {
    let executor = CountingExecutor::new();
    let calls = Arc::clone(&executor.calls);
    let response = transport(executor)
        .handle(request(
            action("shell.execute", "workstation"),
            Some("Test desktop"),
        ))
        .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn authorization_pending_action_never_reaches_executor() {
    let executor = CountingExecutor::new();
    let calls = Arc::clone(&executor.calls);
    let response = transport(executor)
        .handle(request(
            action("demo.protected_action", "demo"),
            Some("Test operator"),
        ))
        .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let body = body_json(response).await;
    assert_eq!(body["status"], "authorization_required");
}

#[tokio::test]
async fn secret_input_is_never_reflected_in_errors() {
    let marker = "sensitive-test-marker";
    let mut body = action("core.health.read", "jarvis-core");
    body["action"]["parameters"]["api_key"] = Value::String(marker.into());
    let response = transport(CountingExecutor::new())
        .handle(request(body, Some("Test desktop")))
        .await;
    let serialized = serde_json::to_string(&body_json(response).await).expect("response JSON");

    assert!(!serialized.contains(marker));
    assert!(!serialized.contains("api_key"));
}

#[tokio::test]
async fn route_methods_are_allowlisted() {
    let response = transport(CountingExecutor::new())
        .handle(
            Request::builder()
                .method(Method::DELETE)
                .uri("/v1/requests")
                .body(Full::new(Bytes::new()))
                .expect("request"),
        )
        .await;

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(response.headers()[header::ALLOW], "POST");
}

struct PendingBody;

impl Body for PendingBody {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        self: Pin<&mut Self>,
        _context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Poll::Pending
    }

    fn is_end_stream(&self) -> bool {
        false
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::new()
    }
}

#[tokio::test]
async fn stalled_body_hits_request_timeout() {
    let executor = CountingExecutor::new();
    let gateway = CoreGateway::new(
        PolicyEngine::default(),
        executor,
        Arc::new(MemoryAuditSink::default()),
    );
    let transport = Transport::with_config(
        gateway,
        TestAuthenticator,
        TransportConfig {
            max_body_bytes: 1024,
            request_timeout: Duration::from_millis(10),
        },
    );
    let request = Request::builder()
        .method(Method::POST)
        .uri("/v1/requests")
        .header(header::AUTHORIZATION, "Test desktop")
        .header(header::CONTENT_TYPE, "application/json")
        .body(PendingBody)
        .expect("request");

    let response = transport.handle(request).await;

    assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
}
