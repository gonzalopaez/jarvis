use crate::{
    AgentHealthStatus, AgentsResponse, ApiError, AuditSink, AuthContext, ComponentHealth,
    CoreGateway, CoreRequest, CoreResponse, EventBus, JarvisState, OperationalHealth,
    ResponseStatus, RestrictedExecutor, SessionStore, SystemHealth, API_VERSION,
};
#[cfg(feature = "network-server")]
use crate::{ConversationService, VoicePipeline, VoicePipelineError};
use bytes::Bytes;
use http::{
    header::{ALLOW, AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, COOKIE, SET_COOKIE},
    HeaderMap, Method, Request, Response, StatusCode,
};
use http_body::Body;
use http_body_util::{BodyExt, Full, Limited};
#[cfg(feature = "network-server")]
use serde::Deserialize;
use serde::Serialize;
#[cfg(feature = "network-server")]
use std::time::Instant;
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(feature = "network-server")]
use crate::EventType;
#[cfg(feature = "network-server")]
use futures_util::{SinkExt, StreamExt};
#[cfg(feature = "network-server")]
use hyper::{body::Incoming, server::conn::http1, service::service_fn};
#[cfg(feature = "network-server")]
use hyper_util::rt::TokioIo;
#[cfg(feature = "network-server")]
use std::convert::Infallible;
#[cfg(feature = "network-server")]
use std::future::Future;
#[cfg(feature = "network-server")]
use tokio::net::TcpListener;

const CSRF_HEADER: &str = "x-jarvis-csrf";
#[cfg(feature = "network-server")]
const MAX_ALERT_AUDIO_REQUEST_BYTES: usize = 8 * 1024;

#[cfg(feature = "network-server")]
const WEBSOCKET_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(25);
#[cfg(feature = "network-server")]
const MAX_WEBSOCKET_INPUT_BYTES: usize = 16 * 1024;
#[cfg(feature = "network-server")]
const MAX_VOICE_CHUNK_BYTES: usize = 64 * 1024;
#[cfg(feature = "network-server")]
const MAX_VOICE_SESSION_BYTES: usize = 16 * 1024 * 1024;
#[cfg(feature = "network-server")]
const MAX_VOICE_PIPELINE_DURATION: Duration = Duration::from_secs(90);

#[cfg(feature = "network-server")]
#[derive(Debug, Default, Serialize)]
struct VoiceTimingLog {
    request_id: String,
    capture_upload_ms: u64,
    stt_ms: u64,
    routing_ms: u64,
    llm_ms: u64,
    tts_ms: u64,
    audio_transfer_ms: u64,
    total_ms: u64,
}

#[cfg(feature = "network-server")]
struct VoiceTimingGuard {
    log: VoiceTimingLog,
    total_started: Instant,
}

#[cfg(feature = "network-server")]
impl VoiceTimingGuard {
    fn new(request_id: String, total_started: Instant) -> Self {
        Self {
            log: VoiceTimingLog {
                request_id,
                capture_upload_ms: elapsed_ms(total_started),
                ..VoiceTimingLog::default()
            },
            total_started,
        }
    }
}

#[cfg(feature = "network-server")]
impl Drop for VoiceTimingGuard {
    fn drop(&mut self) {
        self.log.total_ms = elapsed_ms(self.total_started);
        if let Ok(payload) = serde_json::to_string(&self.log) {
            eprintln!("JARVIS_VOICE_TIMING {payload}");
        }
    }
}

#[cfg(feature = "network-server")]
fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

#[cfg(feature = "network-server")]
pub struct PrivateListener(TcpListener);

pub const DEFAULT_MAX_BODY_BYTES: usize = 32 * 1024;
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
pub const DEFAULT_CONVERSATION_TIMEOUT: Duration = Duration::from_secs(30);

type ResponseBody = Full<Bytes>;

#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub max_body_bytes: usize,
    pub request_timeout: Duration,
    pub conversation_timeout: Duration,
    pub websocket_origin: Option<String>,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            conversation_timeout: DEFAULT_CONVERSATION_TIMEOUT,
            websocket_origin: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindAddressError;

pub trait Authenticator: Send + Sync + 'static {
    fn authenticate(&self, credential: Option<&str>) -> Result<AuthContext, AuthError>;
}

pub struct Transport<E, A, U> {
    gateway: Arc<CoreGateway<E, A>>,
    authenticator: Arc<U>,
    config: TransportConfig,
    events: EventBus,
    sessions: SessionStore,
    codex_configured: bool,
    #[cfg(feature = "network-server")]
    voice: Option<Arc<VoicePipeline>>,
    #[cfg(feature = "network-server")]
    conversation: Option<Arc<ConversationService>>,
}

impl<E, A, U> Clone for Transport<E, A, U> {
    fn clone(&self) -> Self {
        Self {
            gateway: Arc::clone(&self.gateway),
            authenticator: Arc::clone(&self.authenticator),
            config: self.config.clone(),
            events: self.events.clone(),
            sessions: self.sessions.clone(),
            codex_configured: self.codex_configured,
            #[cfg(feature = "network-server")]
            voice: self.voice.clone(),
            #[cfg(feature = "network-server")]
            conversation: self.conversation.clone(),
        }
    }
}

impl<E, A, U> Transport<E, A, U>
where
    E: RestrictedExecutor + Send + Sync + 'static,
    A: AuditSink + Send + Sync + 'static,
    U: Authenticator,
{
    pub fn new(gateway: CoreGateway<E, A>, authenticator: U) -> Self {
        Self::with_config(gateway, authenticator, TransportConfig::default())
    }

    pub fn with_config(
        gateway: CoreGateway<E, A>,
        authenticator: U,
        config: TransportConfig,
    ) -> Self {
        Self {
            gateway: Arc::new(gateway),
            authenticator: Arc::new(authenticator),
            config,
            events: EventBus::default(),
            sessions: SessionStore::default(),
            codex_configured: false,
            #[cfg(feature = "network-server")]
            voice: None,
            #[cfg(feature = "network-server")]
            conversation: None,
        }
    }

    pub fn with_event_bus(mut self, events: EventBus) -> Self {
        self.events = events;
        self
    }

    #[cfg(feature = "network-server")]
    pub fn with_voice_pipeline(mut self, voice: VoicePipeline) -> Self {
        self.voice = Some(Arc::new(voice));
        self
    }

    #[cfg(feature = "network-server")]
    pub fn with_conversation_service(mut self, conversation: ConversationService) -> Self {
        self.conversation = Some(Arc::new(conversation));
        self
    }

    pub fn event_bus(&self) -> EventBus {
        self.events.clone()
    }

    pub fn with_codex_configured(mut self, configured: bool) -> Self {
        self.codex_configured = configured;
        self
    }

    pub fn session_store(&self) -> SessionStore {
        self.sessions.clone()
    }

    pub async fn handle<B>(&self, request: Request<B>) -> Response<ResponseBody>
    where
        B: Body<Data = Bytes> + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let timeout = if request.method() == Method::POST
            && matches!(request.uri().path(), "/v1/requests" | "/api/v1/requests")
        {
            self.config.conversation_timeout
        } else {
            self.config.request_timeout
        };
        match tokio::time::timeout(timeout, self.route(request)).await {
            Ok(response) => response,
            Err(_) => transport_error(
                StatusCode::REQUEST_TIMEOUT,
                "REQUEST_TIMEOUT",
                "Request deadline exceeded",
            ),
        }
    }

    async fn route<B>(&self, request: Request<B>) -> Response<ResponseBody>
    where
        B: Body<Data = Bytes> + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        match (request.method(), request.uri().path()) {
            (&Method::GET, "/ws") => self.handle_websocket(request).await,
            (&Method::GET, "/ws/voice") => self.handle_voice_websocket(request).await,
            (&Method::GET, "/v1/health") => json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "api_version": API_VERSION,
                    "status": "ready"
                }),
            ),
            (&Method::GET, "/api/v1/health") => {
                json_response(StatusCode::OK, &aggregate_health(self.codex_configured))
            }
            (&Method::GET, "/api/v1/agents") => self.handle_agents(request.headers()),
            (&Method::GET, "/api/v1/session") => self.handle_session_status(request.headers()),
            (&Method::POST, "/api/v1/session") => self.handle_session_login(request.headers()),
            (&Method::DELETE, "/api/v1/session") => self.handle_session_logout(request.headers()),
            (&Method::POST, "/v1/requests" | "/api/v1/requests") => {
                self.handle_core_request(request).await
            }
            #[cfg(feature = "network-server")]
            (&Method::POST, "/api/v1/voice/alert") => self.handle_alert_audio(request).await,
            (_, "/v1/health") => method_not_allowed("GET"),
            (_, "/api/v1/health" | "/api/v1/agents") => method_not_allowed("GET"),
            (_, "/api/v1/session") => method_not_allowed("GET, POST, DELETE"),
            (_, "/ws" | "/ws/voice") => method_not_allowed("GET"),
            (_, "/v1/requests" | "/api/v1/requests") => method_not_allowed("POST"),
            (_, "/api/v1/voice/alert") => method_not_allowed("POST"),
            _ => transport_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Route not found"),
        }
    }

    #[cfg(feature = "network-server")]
    async fn handle_alert_audio<B>(&self, request: Request<B>) -> Response<ResponseBody>
    where
        B: Body<Data = Bytes> + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let uses_bearer = request.headers().contains_key(AUTHORIZATION);
        if self.authenticate(request.headers()).is_err() {
            return transport_error(
                StatusCode::UNAUTHORIZED,
                "AUTHENTICATION_REQUIRED",
                "Valid authentication is required",
            );
        }
        if !uses_bearer && !self.valid_session_write(request.headers()) {
            return transport_error(
                StatusCode::FORBIDDEN,
                "CSRF_REJECTED",
                "Session request validation failed",
            );
        }
        if !is_json(request.headers()) {
            return transport_error(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "JSON_REQUIRED",
                "Content-Type must be application/json",
            );
        }
        if content_length_exceeds(request.headers(), MAX_ALERT_AUDIO_REQUEST_BYTES) {
            return transport_error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "PAYLOAD_TOO_LARGE",
                "Alert audio request exceeds the configured limit",
            );
        }
        let Some(voice) = &self.voice else {
            return transport_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "VOICE_SERVICE_UNAVAILABLE",
                "Voice service is unavailable",
            );
        };
        let body = Limited::new(request.into_body(), MAX_ALERT_AUDIO_REQUEST_BYTES).collect();
        let body = match tokio::time::timeout(self.config.request_timeout, body).await {
            Ok(Ok(body)) => body.to_bytes(),
            Ok(Err(_)) => {
                return transport_error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "PAYLOAD_TOO_LARGE",
                    "Alert audio request exceeds the configured limit",
                )
            }
            Err(_) => {
                return transport_error(
                    StatusCode::REQUEST_TIMEOUT,
                    "REQUEST_TIMEOUT",
                    "Request body deadline exceeded",
                )
            }
        };
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct AlertAudioRequest {
            text: String,
        }
        let Ok(payload) = serde_json::from_slice::<AlertAudioRequest>(&body) else {
            return transport_error(
                StatusCode::BAD_REQUEST,
                "INVALID_REQUEST",
                "Alert audio request is invalid",
            );
        };
        if payload.text.trim().is_empty() || payload.text.len() > 2 * 1024 {
            return transport_error(
                StatusCode::BAD_REQUEST,
                "INVALID_REQUEST",
                "Alert text is invalid",
            );
        }
        match voice.synthesize_text(&payload.text).await {
            Ok(audio) => Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, "audio/wav")
                .header("cache-control", "no-store")
                .body(Full::new(Bytes::from(audio)))
                .expect("audio response metadata is valid"),
            Err(_) => transport_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "TTS_UNAVAILABLE",
                "Voice synthesis is unavailable",
            ),
        }
    }

    #[cfg(feature = "network-server")]
    async fn handle_websocket<B>(&self, mut request: Request<B>) -> Response<ResponseBody>
    where
        B: Body<Data = Bytes> + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        if self.authenticate(request.headers()).is_err() {
            return transport_error(
                StatusCode::UNAUTHORIZED,
                "AUTHENTICATION_REQUIRED",
                "Valid authentication is required",
            );
        }
        let Some(expected_origin) = self.config.websocket_origin.as_deref() else {
            return transport_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "WEBSOCKET_DISABLED",
                "WebSocket origin is not configured",
            );
        };
        let origin_matches = request
            .headers()
            .get("origin")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|origin| origin == expected_origin);
        if !origin_matches {
            return transport_error(
                StatusCode::FORBIDDEN,
                "ORIGIN_REJECTED",
                "WebSocket origin is not allowed",
            );
        }
        if !hyper_tungstenite::is_upgrade_request(&request) {
            return transport_error(
                StatusCode::BAD_REQUEST,
                "WEBSOCKET_UPGRADE_REQUIRED",
                "A valid WebSocket upgrade is required",
            );
        }
        let Ok((response, websocket)) = hyper_tungstenite::upgrade(&mut request, None) else {
            return transport_error(
                StatusCode::BAD_REQUEST,
                "WEBSOCKET_UPGRADE_REJECTED",
                "WebSocket upgrade was rejected",
            );
        };
        let events = self.events.clone();
        let codex_configured = self.codex_configured;
        tokio::spawn(async move {
            let Ok(socket) = websocket.await else { return };
            run_websocket(socket, events, codex_configured).await;
        });
        response
    }

    #[cfg(feature = "network-server")]
    async fn handle_voice_websocket<B>(&self, mut request: Request<B>) -> Response<ResponseBody>
    where
        B: Body<Data = Bytes> + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        if self.authenticate(request.headers()).is_err() {
            return transport_error(
                StatusCode::UNAUTHORIZED,
                "AUTHENTICATION_REQUIRED",
                "Valid authentication is required",
            );
        }
        if !self.valid_origin(request.headers()) {
            return transport_error(
                StatusCode::FORBIDDEN,
                "ORIGIN_REJECTED",
                "WebSocket origin is not allowed",
            );
        }
        if !hyper_tungstenite::is_upgrade_request(&request) {
            return transport_error(
                StatusCode::BAD_REQUEST,
                "WEBSOCKET_UPGRADE_REQUIRED",
                "A valid WebSocket upgrade is required",
            );
        }
        let Ok((response, websocket)) = hyper_tungstenite::upgrade(&mut request, None) else {
            return transport_error(
                StatusCode::BAD_REQUEST,
                "WEBSOCKET_UPGRADE_REJECTED",
                "WebSocket upgrade was rejected",
            );
        };
        let events = self.events.clone();
        let voice = self.voice.clone();
        let conversation = self.conversation.clone();
        tokio::spawn(async move {
            let Ok(socket) = websocket.await else { return };
            let _ = tokio::time::timeout(
                MAX_VOICE_PIPELINE_DURATION,
                run_voice_websocket(socket, events, voice, conversation),
            )
            .await;
        });
        response
    }

    #[cfg(not(feature = "network-server"))]
    async fn handle_websocket<B>(&self, _request: Request<B>) -> Response<ResponseBody>
    where
        B: Body<Data = Bytes> + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        transport_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "WEBSOCKET_DISABLED",
            "WebSocket support is not enabled",
        )
    }

    #[cfg(not(feature = "network-server"))]
    async fn handle_voice_websocket<B>(&self, _request: Request<B>) -> Response<ResponseBody>
    where
        B: Body<Data = Bytes> + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        transport_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "WEBSOCKET_DISABLED",
            "WebSocket support is not enabled",
        )
    }

    fn handle_agents(&self, headers: &HeaderMap) -> Response<ResponseBody> {
        if self.authenticate(headers).is_err() {
            return transport_error(
                StatusCode::UNAUTHORIZED,
                "AUTHENTICATION_REQUIRED",
                "Valid authentication is required",
            );
        }
        json_response(
            StatusCode::OK,
            &AgentsResponse {
                api_version: API_VERSION,
                agents: component_health(self.codex_configured),
            },
        )
    }

    fn handle_session_status(&self, headers: &HeaderMap) -> Response<ResponseBody> {
        if self.authenticate(headers).is_err() {
            return transport_error(
                StatusCode::UNAUTHORIZED,
                "AUTHENTICATION_REQUIRED",
                "Valid authentication is required",
            );
        }
        let Some(csrf_token) = headers
            .get(COOKIE)
            .and_then(|value| value.to_str().ok())
            .and_then(|cookie| self.sessions.csrf_token(cookie))
        else {
            return transport_error(
                StatusCode::UNAUTHORIZED,
                "SESSION_REQUIRED",
                "A browser session is required",
            );
        };
        json_response(
            StatusCode::OK,
            &serde_json::json!({
                "api_version": API_VERSION,
                "authenticated": true,
                "csrf_token": csrf_token
            }),
        )
    }

    fn handle_session_login(&self, headers: &HeaderMap) -> Response<ResponseBody> {
        if !self.valid_origin(headers) {
            return transport_error(
                StatusCode::FORBIDDEN,
                "ORIGIN_REJECTED",
                "Request origin is not allowed",
            );
        }
        let auth = match self.authenticator.authenticate(credential(headers)) {
            Ok(auth) if auth.authenticated => auth,
            _ => {
                return transport_error(
                    StatusCode::UNAUTHORIZED,
                    "AUTHENTICATION_REQUIRED",
                    "Valid authentication is required",
                )
            }
        };
        let Some(principal) = auth.principal else {
            return transport_error(
                StatusCode::UNAUTHORIZED,
                "AUTHENTICATION_REQUIRED",
                "Valid authentication is required",
            );
        };
        let issued = match self.sessions.issue(principal) {
            Ok(session) => session,
            Err(_) => {
                return transport_error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "SESSION_UNAVAILABLE",
                    "Session could not be issued",
                )
            }
        };
        let cookie = issued.cookie_header();
        let mut response = json_response(
            StatusCode::CREATED,
            &serde_json::json!({
                "api_version": API_VERSION,
                "authenticated": true,
                "expires_at_ms": issued.expires_at_ms
            }),
        );
        response
            .headers_mut()
            .insert(SET_COOKIE, cookie.parse().expect("issued cookie is valid"));
        response
    }

    fn handle_session_logout(&self, headers: &HeaderMap) -> Response<ResponseBody> {
        if !self.valid_session_write(headers) {
            return transport_error(
                StatusCode::FORBIDDEN,
                "CSRF_REJECTED",
                "Session request validation failed",
            );
        }
        if let Some(cookie) = headers.get(COOKIE).and_then(|value| value.to_str().ok()) {
            self.sessions.revoke_cookie(cookie);
        }
        let mut response = json_response(
            StatusCode::OK,
            &serde_json::json!({
                "api_version": API_VERSION,
                "authenticated": false
            }),
        );
        response.headers_mut().insert(
            SET_COOKIE,
            "jarvis_session=; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=0"
                .parse()
                .expect("static cookie is valid"),
        );
        response
    }

    fn valid_origin(&self, headers: &HeaderMap) -> bool {
        self.config
            .websocket_origin
            .as_deref()
            .is_some_and(|expected| {
                headers.get("origin").and_then(|value| value.to_str().ok()) == Some(expected)
            })
    }

    fn authenticate(&self, headers: &HeaderMap) -> Result<AuthContext, AuthError> {
        let bearer = self.authenticator.authenticate(credential(headers)).ok();
        let session = headers
            .get(COOKIE)
            .and_then(|value| value.to_str().ok())
            .and_then(|cookie| self.sessions.authenticate_cookie(cookie));
        bearer
            .or(session)
            .filter(|auth| auth.authenticated && auth.principal.is_some())
            .ok_or(AuthError)
    }

    async fn handle_core_request<B>(&self, request: Request<B>) -> Response<ResponseBody>
    where
        B: Body<Data = Bytes> + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let uses_bearer = request.headers().contains_key(AUTHORIZATION);
        let auth = match self.authenticate(request.headers()) {
            Ok(auth) => auth,
            _ => {
                return transport_error(
                    StatusCode::UNAUTHORIZED,
                    "AUTHENTICATION_REQUIRED",
                    "Valid authentication is required",
                )
            }
        };

        if !uses_bearer && !self.valid_session_write(request.headers()) {
            return transport_error(
                StatusCode::FORBIDDEN,
                "CSRF_REJECTED",
                "Session request validation failed",
            );
        }

        if !is_json(request.headers()) {
            return transport_error(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "JSON_REQUIRED",
                "Content-Type must be application/json",
            );
        }

        if content_length_exceeds(request.headers(), self.config.max_body_bytes) {
            return transport_error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "PAYLOAD_TOO_LARGE",
                "Request body exceeds the configured limit",
            );
        }

        let body = Limited::new(request.into_body(), self.config.max_body_bytes).collect();
        let collected = match tokio::time::timeout(self.config.request_timeout, body).await {
            Ok(Ok(collected)) => collected,
            Ok(Err(_)) => {
                return transport_error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "PAYLOAD_TOO_LARGE",
                    "Request body exceeds the configured limit",
                )
            }
            Err(_) => {
                return transport_error(
                    StatusCode::REQUEST_TIMEOUT,
                    "REQUEST_TIMEOUT",
                    "Request body deadline exceeded",
                )
            }
        };
        let core_request = match serde_json::from_slice::<CoreRequest>(&collected.to_bytes()) {
            Ok(request) => request,
            Err(_) => {
                return transport_error(
                    StatusCode::BAD_REQUEST,
                    "INVALID_JSON",
                    "Request body is not valid for the v1 contract",
                )
            }
        };

        let response = if core_request.kind == "conversation" {
            if crate::validate_request(&core_request).is_err() {
                self.gateway.handle(&auth, core_request)
            } else {
                #[cfg(feature = "network-server")]
                if let Some(conversation) = &self.conversation {
                    conversation.handle(&core_request).await
                } else {
                    self.gateway.handle(&auth, core_request)
                }
                #[cfg(not(feature = "network-server"))]
                self.gateway.handle(&auth, core_request)
            }
        } else {
            self.gateway.handle(&auth, core_request)
        };
        let status = status_for(&response);
        json_response(status, &response)
    }

    fn valid_session_write(&self, headers: &HeaderMap) -> bool {
        let origin_matches = self.valid_origin(headers);
        let cookie = headers.get(COOKIE).and_then(|value| value.to_str().ok());
        let csrf = headers
            .get(CSRF_HEADER)
            .and_then(|value| value.to_str().ok());
        origin_matches
            && cookie
                .zip(csrf)
                .is_some_and(|(cookie, csrf)| self.sessions.validate_csrf(cookie, csrf))
    }
}

#[cfg(feature = "network-server")]
async fn run_websocket(
    socket: hyper_tungstenite::HyperWebsocketStream,
    events: EventBus,
    codex_configured: bool,
) {
    use hyper_tungstenite::tungstenite::Message;
    use serde_json::json;

    let (mut sender, mut receiver) = socket.split();
    let snapshot = events.build(
        EventType::SystemSnapshot,
        None,
        serde_json::to_value(aggregate_health(codex_configured)).unwrap_or_else(|_| json!({})),
    );
    if sender.send(envelope_message(&snapshot)).await.is_err() {
        return;
    }
    for event in events.recent_security_events() {
        if sender.send(envelope_message(&event)).await.is_err() {
            return;
        }
    }

    let mut subscription = events.subscribe();
    let mut heartbeat = tokio::time::interval(WEBSOCKET_HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    heartbeat.tick().await;

    loop {
        tokio::select! {
            incoming = receiver.next() => match incoming {
                Some(Ok(Message::Pong(_))) => {}
                Some(Ok(Message::Ping(payload))) => {
                    if sender.send(Message::Pong(payload)).await.is_err() { break; }
                }
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                Some(Ok(Message::Text(text))) if text.len() > MAX_WEBSOCKET_INPUT_BYTES => break,
                Some(Ok(Message::Binary(bytes))) if bytes.len() > MAX_WEBSOCKET_INPUT_BYTES => break,
                Some(Ok(Message::Text(_) | Message::Binary(_))) => break,
                Some(Ok(_)) => {}
            },
            event = subscription.recv() => match event {
                Ok(event) => {
                    if sender.send(envelope_message(&event)).await.is_err() { break; }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    let resync = events.build(
                        EventType::SystemResyncRequired,
                        None,
                        json!({ "skipped": skipped }),
                    );
                    if sender.send(envelope_message(&resync)).await.is_err() { break; }
                    let snapshot = events.build(
                        EventType::SystemSnapshot,
                        None,
                        serde_json::to_value(aggregate_health(codex_configured)).unwrap_or_else(|_| json!({})),
                    );
                    if sender.send(envelope_message(&snapshot)).await.is_err() { break; }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            },
            _ = heartbeat.tick() => {
                let event = events.build(EventType::SystemHeartbeat, None, json!({}));
                if sender.send(envelope_message(&event)).await.is_err() { break; }
            }
        }
    }
}

#[cfg(feature = "network-server")]
async fn run_voice_websocket(
    socket: hyper_tungstenite::HyperWebsocketStream,
    events: EventBus,
    voice: Option<Arc<VoicePipeline>>,
    conversation: Option<Arc<ConversationService>>,
) {
    use hyper_tungstenite::tungstenite::Message;
    use serde_json::{json, Value};

    let (mut sender, mut receiver) = socket.split();
    let mut session_id: Option<String> = None;
    let mut session_started: Option<Instant> = None;
    let mut received_bytes = 0usize;
    let mut audio = Vec::new();
    let mut mime_type: Option<String> = None;
    while let Some(incoming) = receiver.next().await {
        let Ok(message) = incoming else { break };
        match message {
            Message::Ping(payload) => {
                if sender.send(Message::Pong(payload)).await.is_err() {
                    break;
                }
            }
            Message::Text(text) if text.len() <= 1_024 => {
                let Ok(value) = serde_json::from_str::<Value>(text.as_str()) else {
                    break;
                };
                let Some(object) = value.as_object() else {
                    break;
                };
                let message_type = object.get("type").and_then(Value::as_str);
                if session_id.is_none() && message_type == Some("voice.session.start") {
                    let id = object.get("session_id").and_then(Value::as_str);
                    let mime = object.get("mime_type").and_then(Value::as_str);
                    let valid_id = id.is_some_and(valid_voice_session_id);
                    let valid_mime = matches!(
                        mime,
                        Some("audio/webm;codecs=opus" | "audio/ogg;codecs=opus")
                    );
                    if object.len() != 4
                        || object.get("version").and_then(Value::as_str) != Some("v1")
                        || !valid_id
                        || !valid_mime
                    {
                        break;
                    }
                    let id = id.expect("validated id").to_owned();
                    events.publish(
                        EventType::VoiceSessionStarted,
                        Some(id.clone()),
                        json!({ "mime_type": mime }),
                    );
                    events.publish(
                        EventType::JarvisStateChanged,
                        Some(id.clone()),
                        json!({ "state": "LISTENING" }),
                    );
                    session_id = Some(id);
                    session_started = Some(Instant::now());
                    mime_type = mime.map(str::to_owned);
                    let ready = json!({ "version": "v1", "type": "voice.session.ready", "max_chunk_bytes": MAX_VOICE_CHUNK_BYTES });
                    if sender
                        .send(Message::Text(ready.to_string().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                } else if message_type == Some("voice.session.stop") {
                    let matches_session =
                        session_id.as_deref() == object.get("session_id").and_then(Value::as_str);
                    if object.len() != 3
                        || object.get("version").and_then(Value::as_str) != Some("v1")
                        || !matches_session
                    {
                        break;
                    }
                    let id = session_id.take().expect("validated active voice session");
                    let mut timing = VoiceTimingGuard::new(
                        id.clone(),
                        session_started.take().unwrap_or_else(Instant::now),
                    );
                    let Some(pipeline) = voice.as_ref() else {
                        events.publish(EventType::VoiceSessionFailed, Some(id.clone()), json!({ "code": "VOICE_SERVICE_UNAVAILABLE", "received_bytes": received_bytes }));
                        let unavailable = json!({ "version": "v1", "type": "voice.session.unavailable", "code": "VOICE_SERVICE_UNAVAILABLE" });
                        let _ = sender
                            .send(Message::Text(unavailable.to_string().into()))
                            .await;
                        let _ = sender.send(Message::Close(None)).await;
                        return;
                    };
                    if audio.is_empty() {
                        let failed = json!({ "version": "v1", "type": "voice.session.failed", "code": "EMPTY_AUDIO" });
                        let _ = sender.send(Message::Text(failed.to_string().into())).await;
                        return;
                    }
                    events.publish(
                        EventType::JarvisStateChanged,
                        Some(id.clone()),
                        json!({ "state": "THINKING" }),
                    );
                    let captured_audio = std::mem::take(&mut audio);
                    let result = if let Some(conversation) = conversation.as_ref() {
                        process_routed_voice(
                            pipeline,
                            conversation,
                            &id,
                            mime_type.as_deref().unwrap_or("audio/webm;codecs=opus"),
                            captured_audio,
                            &mut timing.log,
                        )
                        .await
                    } else {
                        process_unrouted_voice(
                            pipeline,
                            mime_type.as_deref().unwrap_or("audio/webm;codecs=opus"),
                            captured_audio,
                            &mut timing.log,
                        )
                        .await
                    };
                    match result {
                        Ok(result) => {
                            let transcript = json!({ "version": "v1", "type": "voice.transcript", "text": result.transcript });
                            let response = json!({ "version": "v1", "type": "voice.response", "text": result.response });
                            let output = json!({ "version": "v1", "type": "voice.output.start", "mime_type": "audio/wav", "bytes": result.audio.len() });
                            if sender
                                .send(Message::Text(transcript.to_string().into()))
                                .await
                                .is_err()
                            {
                                return;
                            }
                            if sender
                                .send(Message::Text(response.to_string().into()))
                                .await
                                .is_err()
                            {
                                return;
                            }
                            events.publish(
                                EventType::JarvisStateChanged,
                                Some(id.clone()),
                                json!({ "state": "SPEAKING" }),
                            );
                            let transfer_started = Instant::now();
                            if sender
                                .send(Message::Text(output.to_string().into()))
                                .await
                                .is_err()
                            {
                                return;
                            }
                            if sender
                                .send(Message::Binary(result.audio.into()))
                                .await
                                .is_err()
                            {
                                return;
                            }
                            let complete =
                                json!({ "version": "v1", "type": "voice.output.complete" });
                            let _ = sender
                                .send(Message::Text(complete.to_string().into()))
                                .await;
                            timing.log.audio_transfer_ms = elapsed_ms(transfer_started);
                        }
                        Err(error) => {
                            let code = match error {
                                VoicePipelineError::SpeechRecognitionUnavailable => {
                                    "STT_UNAVAILABLE"
                                }
                                VoicePipelineError::ModelUnavailable => "MODEL_UNAVAILABLE",
                                VoicePipelineError::SpeechSynthesisUnavailable => "TTS_UNAVAILABLE",
                                VoicePipelineError::InvalidConfiguration
                                | VoicePipelineError::InvalidResponse => "VOICE_PIPELINE_ERROR",
                            };
                            events.publish(
                                EventType::VoiceSessionFailed,
                                Some(id.clone()),
                                json!({ "code": code }),
                            );
                            events.publish(
                                EventType::JarvisStateChanged,
                                Some(id.clone()),
                                json!({ "state": "WARNING" }),
                            );
                            let failed = json!({ "version": "v1", "type": "voice.session.failed", "code": code });
                            eprintln!("JARVIS_VOICE session={} outcome=failed code={}", id, code);
                            let _ = sender.send(Message::Text(failed.to_string().into())).await;
                        }
                    }
                    let _ = sender.send(Message::Close(None)).await;
                    return;
                } else {
                    break;
                }
            }
            Message::Binary(bytes)
                if session_id.is_some() && bytes.len() <= MAX_VOICE_CHUNK_BYTES =>
            {
                let Some(total) = received_bytes.checked_add(bytes.len()) else {
                    break;
                };
                if total > MAX_VOICE_SESSION_BYTES {
                    break;
                }
                received_bytes = total;
                audio.extend_from_slice(&bytes);
            }
            Message::Close(_) => return,
            _ => break,
        }
    }
    if let Some(id) = session_id {
        events.publish(
            EventType::VoiceSessionFailed,
            Some(id),
            json!({ "code": "VOICE_PROTOCOL_ABORTED" }),
        );
    }
}

#[cfg(feature = "network-server")]
async fn process_routed_voice(
    pipeline: &VoicePipeline,
    conversation: &ConversationService,
    session_id: &str,
    mime_type: &str,
    audio: Vec<u8>,
    timings: &mut VoiceTimingLog,
) -> Result<crate::VoicePipelineResult, VoicePipelineError> {
    let stt_started = Instant::now();
    let transcript = pipeline.transcribe_audio(mime_type, audio).await;
    timings.stt_ms = elapsed_ms(stt_started);
    let transcript = transcript?;
    let request = CoreRequest {
        api_version: API_VERSION.into(),
        request_id: session_id.into(),
        session_id: session_id.into(),
        kind: "conversation".into(),
        message: Some(transcript.clone()),
        action: None,
        authorization: None,
    };
    let (response, conversation_timings) = conversation.handle_with_timings(&request).await;
    timings.routing_ms = conversation_timings.routing_ms;
    timings.llm_ms = conversation_timings.llm_ms;
    if response.status != ResponseStatus::Completed {
        return Err(VoicePipelineError::ModelUnavailable);
    }
    let response_text = response
        .data
        .and_then(|data| {
            data.get("message")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .filter(|text| !text.trim().is_empty())
        .ok_or(VoicePipelineError::InvalidResponse)?;
    let tts_started = Instant::now();
    let output = pipeline.synthesize_text(&response_text).await;
    timings.tts_ms = elapsed_ms(tts_started);
    let output = output?;
    Ok(crate::VoicePipelineResult {
        transcript,
        response: response_text,
        audio: output,
    })
}

#[cfg(feature = "network-server")]
async fn process_unrouted_voice(
    pipeline: &VoicePipeline,
    mime_type: &str,
    audio: Vec<u8>,
    timings: &mut VoiceTimingLog,
) -> Result<crate::VoicePipelineResult, VoicePipelineError> {
    let stt_started = Instant::now();
    let transcript = pipeline.transcribe_audio(mime_type, audio).await;
    timings.stt_ms = elapsed_ms(stt_started);
    let transcript = transcript?;

    let llm_started = Instant::now();
    let response = pipeline.complete_text(&transcript, "jarvis-fast").await;
    timings.llm_ms = elapsed_ms(llm_started);
    let response = response?;

    let tts_started = Instant::now();
    let audio = pipeline.synthesize_text(&response).await;
    timings.tts_ms = elapsed_ms(tts_started);
    Ok(crate::VoicePipelineResult {
        transcript,
        response,
        audio: audio?,
    })
}

#[cfg(feature = "network-server")]
fn valid_voice_session_id(value: &str) -> bool {
    (8..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[cfg(feature = "network-server")]
fn envelope_message(event: &crate::EventEnvelope) -> hyper_tungstenite::tungstenite::Message {
    let serialized = serde_json::to_string(event).unwrap_or_else(|_| {
        "{\"event_version\":\"v1\",\"type\":\"system.error\",\"payload\":{}}".into()
    });
    hyper_tungstenite::tungstenite::Message::Text(serialized.into())
}

fn aggregate_health(codex_configured: bool) -> SystemHealth {
    SystemHealth {
        api_version: API_VERSION,
        status: OperationalHealth::Degraded,
        state: JarvisState::Idle,
        components: component_health(codex_configured),
    }
}

fn component_health(codex_configured: bool) -> Vec<ComponentHealth> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    let core = ComponentHealth {
        id: "core",
        label: "JARVIS CORE",
        status: OperationalHealth::Healthy,
        agent_status: AgentHealthStatus::Ready,
        version: env!("CARGO_PKG_VERSION"),
        latency_ms: None,
        last_seen_ms: Some(now),
        error: None,
    };
    let codex = ComponentHealth {
        id: "codex",
        label: "CODEX AGENT",
        status: if codex_configured {
            OperationalHealth::Healthy
        } else {
            OperationalHealth::Unavailable
        },
        agent_status: if codex_configured {
            AgentHealthStatus::Ready
        } else {
            AgentHealthStatus::Offline
        },
        version: if codex_configured {
            "sdk"
        } else {
            "not_connected"
        },
        latency_ms: None,
        last_seen_ms: codex_configured.then_some(now),
        error: (!codex_configured).then_some("not_connected"),
    };
    let unavailable = [
        ("voice", "VOICE SERVICE", "not_connected"),
        ("mcp", "MCP GATEWAY", "not_connected"),
        ("n8n", "N8N", "not_connected"),
        ("wazuh", "WAZUH AGENT", "not_connected"),
        // Proxmox Agent has no network-reachable health surface today (it runs
        // as a stdin/stdout MCP subprocess, not a persistent daemon), so it is
        // never updated by a poller. This is a distinct, permanent error code
        // from "not_connected" so the raw API response doesn't imply a check
        // that isn't actually happening.
        ("proxmox", "PROXMOX AGENT", "not_instrumented"),
    ]
    .into_iter()
    .map(|(id, label, error)| ComponentHealth {
        id,
        label,
        status: OperationalHealth::Unavailable,
        agent_status: AgentHealthStatus::Offline,
        version: "not_connected",
        latency_ms: None,
        last_seen_ms: None,
        error: Some(error),
    });
    std::iter::once(core)
        .chain(std::iter::once(codex))
        .chain(unavailable)
        .collect()
}

#[cfg(feature = "network-server")]
pub async fn bind_private(address: SocketAddr) -> std::io::Result<PrivateListener> {
    validate_bind_address(address).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "JARVIS Core may bind only to loopback or private addresses",
        )
    })?;
    TcpListener::bind(address).await.map(PrivateListener)
}

pub fn validate_bind_address(address: SocketAddr) -> Result<(), BindAddressError> {
    let allowed = match address.ip() {
        IpAddr::V4(ip) => ip.is_loopback() || ip.is_private(),
        IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local(),
    };
    if allowed {
        Ok(())
    } else {
        Err(BindAddressError)
    }
}

#[cfg(feature = "network-server")]
pub async fn serve<E, A, U>(
    listener: PrivateListener,
    transport: Transport<E, A, U>,
) -> std::io::Result<()>
where
    E: RestrictedExecutor + Send + Sync + 'static,
    A: AuditSink + Send + Sync + 'static,
    U: Authenticator,
{
    serve_until(listener, transport, std::future::pending()).await
}

#[cfg(feature = "network-server")]
pub async fn serve_until<E, A, U, S>(
    listener: PrivateListener,
    transport: Transport<E, A, U>,
    shutdown: S,
) -> std::io::Result<()>
where
    E: RestrictedExecutor + Send + Sync + 'static,
    A: AuditSink + Send + Sync + 'static,
    U: Authenticator,
    S: Future<Output = ()>,
{
    let listener = listener.0;
    tokio::pin!(shutdown);
    loop {
        let accepted = tokio::select! {
            result = listener.accept() => result,
            () = &mut shutdown => return Ok(()),
        };
        let (stream, _) = accepted?;
        let io = TokioIo::new(stream);
        let service_transport = transport.clone();
        tokio::spawn(async move {
            let service = service_fn(move |request: Request<Incoming>| {
                let request_transport = service_transport.clone();
                async move { Ok::<_, Infallible>(request_transport.handle(request).await) }
            });
            let _ = http1::Builder::new()
                .serve_connection(io, service)
                .with_upgrades()
                .await;
        });
    }
}

fn credential(headers: &HeaderMap) -> Option<&str> {
    headers.get(AUTHORIZATION)?.to_str().ok()
}

fn is_json(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
}

fn content_length_exceeds(headers: &HeaderMap, limit: usize) -> bool {
    headers
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|length| length > limit)
}

fn status_for(response: &CoreResponse) -> StatusCode {
    match response.status {
        ResponseStatus::Completed => StatusCode::OK,
        ResponseStatus::AuthorizationRequired | ResponseStatus::Denied => StatusCode::FORBIDDEN,
        ResponseStatus::Rejected => StatusCode::UNPROCESSABLE_ENTITY,
    }
}

fn method_not_allowed(allowed: &'static str) -> Response<ResponseBody> {
    let mut response = transport_error(
        StatusCode::METHOD_NOT_ALLOWED,
        "METHOD_NOT_ALLOWED",
        "HTTP method is not allowed for this route",
    );
    response
        .headers_mut()
        .insert(ALLOW, allowed.parse().expect("static method is valid"));
    response
}

#[derive(Serialize)]
struct TransportError {
    api_version: &'static str,
    status: &'static str,
    error: ApiError,
}

fn transport_error(
    status: StatusCode,
    code: &'static str,
    message: &'static str,
) -> Response<ResponseBody> {
    json_response(
        status,
        &TransportError {
            api_version: API_VERSION,
            status: "rejected",
            error: ApiError { code, message },
        },
    )
}

fn json_response<T: Serialize>(status: StatusCode, value: &T) -> Response<ResponseBody> {
    let body = serde_json::to_vec(value)
        .unwrap_or_else(|_| b"{\"status\":\"rejected\",\"error\":{\"code\":\"SERIALIZATION_ERROR\",\"message\":\"Response unavailable\"}}".to_vec());
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json")
        .header("cache-control", "no-store")
        .header("x-content-type-options", "nosniff")
        .body(Full::new(Bytes::from(body)))
        .expect("static response metadata is valid")
}

#[cfg(all(test, feature = "network-server"))]
mod voice_timing_tests {
    use super::VoiceTimingLog;
    use serde_json::{json, Value};
    use std::collections::HashSet;

    #[test]
    fn voice_timing_log_contains_only_request_id_and_numeric_timings() {
        let log = VoiceTimingLog {
            request_id: "voice-request-42".into(),
            capture_upload_ms: 1,
            stt_ms: 2,
            routing_ms: 3,
            llm_ms: 4,
            tts_ms: 5,
            audio_transfer_ms: 6,
            total_ms: 21,
        };
        let value = serde_json::to_value(log).expect("timing log serializes");
        let object = value.as_object().expect("timing log is an object");
        let expected = HashSet::from([
            "request_id",
            "capture_upload_ms",
            "stt_ms",
            "routing_ms",
            "llm_ms",
            "tts_ms",
            "audio_transfer_ms",
            "total_ms",
        ]);

        assert_eq!(
            object.keys().map(String::as_str).collect::<HashSet<_>>(),
            expected
        );
        assert_eq!(object.get("request_id"), Some(&json!("voice-request-42")));
        assert!(object
            .iter()
            .filter(|(key, _)| key.as_str() != "request_id")
            .all(|(_, value)| matches!(value, Value::Number(number) if number.is_u64())));
        assert!(!object.keys().any(|key| matches!(
            key.as_str(),
            "audio" | "text" | "transcript" | "response" | "message" | "prompt" | "error"
        )));
    }
}
