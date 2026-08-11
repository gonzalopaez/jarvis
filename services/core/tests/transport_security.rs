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
    request_at("/v1/requests", body, credential)
}

fn request_at(path: &str, body: Value, credential: Option<&str>) -> Request<Full<Bytes>> {
    let mut builder = Request::builder()
        .method(Method::POST)
        .uri(path)
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

#[tokio::test]
async fn versioned_health_reports_real_and_unavailable_components() {
    let response = transport(CountingExecutor::new())
        .handle(
            Request::builder()
                .uri("/api/v1/health")
                .body(Full::new(Bytes::new()))
                .expect("request"),
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["api_version"], "v1");
    assert_eq!(body["status"], "degraded");
    assert_eq!(body["state"], "IDLE");
    assert_eq!(body["components"].as_array().expect("components").len(), 8);
    assert_eq!(body["components"][0]["id"], "core");
    assert_eq!(body["components"][0]["status"], "healthy");
    assert_eq!(body["components"][1]["status"], "unavailable");
}

#[tokio::test]
async fn agent_inventory_requires_authentication() {
    let anonymous = transport(CountingExecutor::new())
        .handle(
            Request::builder()
                .uri("/api/v1/agents")
                .body(Full::new(Bytes::new()))
                .expect("request"),
        )
        .await;
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

    let authenticated = transport(CountingExecutor::new())
        .handle(
            Request::builder()
                .uri("/api/v1/agents")
                .header(header::AUTHORIZATION, "Test desktop")
                .body(Full::new(Bytes::new()))
                .expect("request"),
        )
        .await;
    assert_eq!(authenticated.status(), StatusCode::OK);
    let body = body_json(authenticated).await;
    assert_eq!(body["agents"].as_array().expect("agents").len(), 8);
}

#[tokio::test]
async fn versioned_request_route_preserves_the_core_contract() {
    let response = transport(CountingExecutor::new())
        .handle(request_at(
            "/api/v1/requests",
            conversation(),
            Some("Test desktop"),
        ))
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_json(response).await["data"]["mode"], "mock");
}

#[cfg(feature = "network-server")]
fn websocket_request(credential: Option<&str>, origin: Option<&str>) -> Request<Full<Bytes>> {
    let mut builder = Request::builder()
        .uri("/ws")
        .header(header::CONNECTION, "upgrade")
        .header(header::UPGRADE, "websocket")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==");
    if let Some(credential) = credential {
        builder = builder.header(header::AUTHORIZATION, credential);
    }
    if let Some(origin) = origin {
        builder = builder.header(header::ORIGIN, origin);
    }
    builder.body(Full::new(Bytes::new())).expect("request")
}

fn websocket_transport(
    origin: Option<&str>,
) -> Transport<CountingExecutor, Arc<MemoryAuditSink>, TestAuthenticator> {
    let gateway = CoreGateway::new(
        PolicyEngine::default(),
        CountingExecutor::new(),
        Arc::new(MemoryAuditSink::default()),
    );
    Transport::with_config(
        gateway,
        TestAuthenticator,
        TransportConfig {
            websocket_origin: origin.map(str::to_owned),
            ..TransportConfig::default()
        },
    )
}

#[cfg(feature = "network-server")]
#[tokio::test]
async fn websocket_requires_authentication_and_configured_exact_origin() {
    let anonymous = websocket_transport(Some("https://jarvis.example.internal"))
        .handle(websocket_request(
            None,
            Some("https://jarvis.example.internal"),
        ))
        .await;
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

    let disabled = websocket_transport(None)
        .handle(websocket_request(
            Some("Test desktop"),
            Some("https://jarvis.example.internal"),
        ))
        .await;
    assert_eq!(disabled.status(), StatusCode::SERVICE_UNAVAILABLE);

    let rejected = websocket_transport(Some("https://jarvis.example.internal"))
        .handle(websocket_request(
            Some("Test desktop"),
            Some("https://evil.example"),
        ))
        .await;
    assert_eq!(rejected.status(), StatusCode::FORBIDDEN);

    let accepted = websocket_transport(Some("https://jarvis.example.internal"))
        .handle(websocket_request(
            Some("Test desktop"),
            Some("https://jarvis.example.internal"),
        ))
        .await;
    assert_eq!(accepted.status(), StatusCode::SWITCHING_PROTOCOLS);
}

#[cfg(feature = "network-server")]
#[tokio::test]
async fn voice_websocket_requires_session_and_exact_origin() {
    let transport = websocket_transport(Some("https://jarvis.example.internal"));
    let mut anonymous = websocket_request(None, Some("https://jarvis.example.internal"));
    *anonymous.uri_mut() = "/ws/voice".parse().expect("voice uri");
    assert_eq!(
        transport.handle(anonymous).await.status(),
        StatusCode::UNAUTHORIZED
    );

    let mut wrong_origin = websocket_request(Some("Test desktop"), Some("https://evil.example"));
    *wrong_origin.uri_mut() = "/ws/voice".parse().expect("voice uri");
    assert_eq!(
        transport.handle(wrong_origin).await.status(),
        StatusCode::FORBIDDEN
    );

    let mut accepted = websocket_request(
        Some("Test desktop"),
        Some("https://jarvis.example.internal"),
    );
    *accepted.uri_mut() = "/ws/voice".parse().expect("voice uri");
    assert_eq!(
        transport.handle(accepted).await.status(),
        StatusCode::SWITCHING_PROTOCOLS
    );
}

#[cfg(feature = "network-server")]
#[tokio::test]
async fn opaque_session_cookie_authenticates_api_and_websocket() {
    let transport = websocket_transport(Some("https://jarvis.example.internal"));
    let issued = transport
        .session_store()
        .issue(jarvis_core::Principal {
            subject: "browser:test".into(),
            roles: vec!["desktop".into()],
        })
        .expect("session");
    let cookie = issued.cookie_header();

    let status = transport
        .handle(
            Request::builder()
                .uri("/api/v1/session")
                .header(header::COOKIE, &cookie)
                .body(Full::new(Bytes::new()))
                .expect("request"),
        )
        .await;
    assert_eq!(status.status(), StatusCode::OK);
    assert_eq!(body_json(status).await["authenticated"], true);

    let without_csrf = request_at("/api/v1/requests", conversation(), None);
    let (mut parts, body) = without_csrf.into_parts();
    parts
        .headers
        .insert(header::COOKIE, cookie.parse().expect("cookie"));
    parts.headers.insert(
        header::ORIGIN,
        "https://jarvis.example.internal".parse().expect("origin"),
    );
    let rejected = transport.handle(Request::from_parts(parts, body)).await;
    assert_eq!(rejected.status(), StatusCode::FORBIDDEN);

    let csrf = transport.session_store().csrf_token(&cookie).expect("csrf");
    let with_csrf = request_at("/api/v1/requests", conversation(), None);
    let (mut parts, body) = with_csrf.into_parts();
    parts
        .headers
        .insert(header::COOKIE, cookie.parse().expect("cookie"));
    parts.headers.insert(
        header::ORIGIN,
        "https://jarvis.example.internal".parse().expect("origin"),
    );
    parts
        .headers
        .insert("x-jarvis-csrf", csrf.parse().expect("csrf"));
    let accepted_request = transport.handle(Request::from_parts(parts, body)).await;
    assert_eq!(accepted_request.status(), StatusCode::OK);

    let websocket = transport
        .handle(
            Request::builder()
                .uri("/ws")
                .header(header::COOKIE, cookie)
                .header(header::ORIGIN, "https://jarvis.example.internal")
                .header(header::CONNECTION, "upgrade")
                .header(header::UPGRADE, "websocket")
                .header("sec-websocket-version", "13")
                .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
                .body(Full::new(Bytes::new()))
                .expect("request"),
        )
        .await;
    assert_eq!(websocket.status(), StatusCode::SWITCHING_PROTOCOLS);
}

#[tokio::test]
async fn browser_login_requires_exact_origin_and_issues_hardened_cookie() {
    let transport = websocket_transport(Some("https://jarvis.example.internal"));
    let login = |credential: Option<&str>, origin: Option<&str>| {
        let mut builder = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/session");
        if let Some(value) = credential {
            builder = builder.header(header::AUTHORIZATION, value);
        }
        if let Some(value) = origin {
            builder = builder.header(header::ORIGIN, value);
        }
        builder.body(Full::new(Bytes::new())).expect("request")
    };

    let wrong_origin = transport
        .handle(login(Some("Test desktop"), Some("https://evil.example")))
        .await;
    assert_eq!(wrong_origin.status(), StatusCode::FORBIDDEN);

    let invalid = transport
        .handle(login(
            Some("Test invalid"),
            Some("https://jarvis.example.internal"),
        ))
        .await;
    assert_eq!(invalid.status(), StatusCode::UNAUTHORIZED);

    let accepted = transport
        .handle(login(
            Some("Test desktop"),
            Some("https://jarvis.example.internal"),
        ))
        .await;
    assert_eq!(accepted.status(), StatusCode::CREATED);
    let cookie = accepted
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .expect("session cookie");
    assert!(cookie.contains("Secure"));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Strict"));
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

#[cfg(feature = "network-server")]
#[tokio::test]
async fn alert_audio_requires_csrf_for_cookie_sessions_and_json_for_bearer() {
    let transport = websocket_transport(Some("https://jarvis.example.internal"));
    let issued = transport
        .session_store()
        .issue(jarvis_core::Principal {
            subject: "browser:test".into(),
            roles: vec!["desktop".into()],
        })
        .expect("session");
    let cookie = issued.cookie_header();

    let without_csrf = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/voice/alert")
        .header(header::COOKIE, &cookie)
        .header(header::ORIGIN, "https://jarvis.example.internal")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from_static(
            br#"{"text":"Alerta critica"}"#,
        )))
        .expect("request");
    assert_eq!(
        transport.handle(without_csrf).await.status(),
        StatusCode::FORBIDDEN
    );

    let wrong_type = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/voice/alert")
        .header(header::AUTHORIZATION, "Test desktop")
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Full::new(Bytes::from_static(b"alert")))
        .expect("request");
    assert_eq!(
        transport.handle(wrong_type).await.status(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );

    let csrf = transport.session_store().csrf_token(&cookie).expect("csrf");
    let accepted_boundary = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/voice/alert")
        .header(header::COOKIE, cookie)
        .header(header::ORIGIN, "https://jarvis.example.internal")
        .header("x-jarvis-csrf", csrf)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from_static(
            br#"{"text":"Alerta critica"}"#,
        )))
        .expect("request");
    assert_eq!(
        transport.handle(accepted_boundary).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[cfg(feature = "network-server")]
#[tokio::test]
async fn alert_audio_rejects_declared_oversized_bodies_before_voice_dispatch() {
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/voice/alert")
        .header(header::AUTHORIZATION, "Test desktop")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CONTENT_LENGTH, "9000")
        .body(Full::new(Bytes::from_static(br#"{"text":"alert"}"#)))
        .expect("request");

    assert_eq!(
        transport(CountingExecutor::new())
            .handle(request)
            .await
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
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
            ..TransportConfig::default()
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
            action("security.host.isolate", "host-01"),
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
            ..TransportConfig::default()
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
