use crate::{
    ApiError, AuditSink, AuthContext, CoreGateway, CoreRequest, CoreResponse, ResponseStatus,
    RestrictedExecutor, API_VERSION,
};
use bytes::Bytes;
use http::{
    header::{ALLOW, AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE},
    HeaderMap, Method, Request, Response, StatusCode,
};
use http_body::Body;
use http_body_util::{BodyExt, Full, Limited};
use serde::Serialize;
use std::{sync::Arc, time::Duration};

#[cfg(feature = "network-server")]
use hyper::{body::Incoming, server::conn::http1, service::service_fn};
#[cfg(feature = "network-server")]
use hyper_util::rt::TokioIo;
#[cfg(feature = "network-server")]
use std::convert::Infallible;
#[cfg(feature = "network-server")]
use tokio::net::TcpListener;

pub const DEFAULT_MAX_BODY_BYTES: usize = 32 * 1024;
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

type ResponseBody = Full<Bytes>;

#[derive(Debug, Clone, Copy)]
pub struct TransportConfig {
    pub max_body_bytes: usize,
    pub request_timeout: Duration,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthError;

pub trait Authenticator: Send + Sync + 'static {
    fn authenticate(&self, credential: Option<&str>) -> Result<AuthContext, AuthError>;
}

pub struct Transport<E, A, U> {
    gateway: Arc<CoreGateway<E, A>>,
    authenticator: Arc<U>,
    config: TransportConfig,
}

impl<E, A, U> Clone for Transport<E, A, U> {
    fn clone(&self) -> Self {
        Self {
            gateway: Arc::clone(&self.gateway),
            authenticator: Arc::clone(&self.authenticator),
            config: self.config,
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
        }
    }

    pub async fn handle<B>(&self, request: Request<B>) -> Response<ResponseBody>
    where
        B: Body<Data = Bytes> + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        match tokio::time::timeout(self.config.request_timeout, self.route(request)).await {
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
            (&Method::GET, "/v1/health") => json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "api_version": API_VERSION,
                    "status": "ready"
                }),
            ),
            (&Method::POST, "/v1/requests") => self.handle_core_request(request).await,
            (_, "/v1/health") => method_not_allowed("GET"),
            (_, "/v1/requests") => method_not_allowed("POST"),
            _ => transport_error(StatusCode::NOT_FOUND, "NOT_FOUND", "Route not found"),
        }
    }

    async fn handle_core_request<B>(&self, request: Request<B>) -> Response<ResponseBody>
    where
        B: Body<Data = Bytes> + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let credential = credential(request.headers());
        let auth = match self.authenticator.authenticate(credential) {
            Ok(auth) if auth.authenticated && auth.principal.is_some() => auth,
            _ => {
                return transport_error(
                    StatusCode::UNAUTHORIZED,
                    "AUTHENTICATION_REQUIRED",
                    "Valid authentication is required",
                )
            }
        };

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

        let collected = match Limited::new(request.into_body(), self.config.max_body_bytes)
            .collect()
            .await
        {
            Ok(collected) => collected,
            Err(_) => {
                return transport_error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "PAYLOAD_TOO_LARGE",
                    "Request body exceeds the configured limit",
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

        let response = self.gateway.handle(&auth, core_request);
        let status = status_for(&response);
        json_response(status, &response)
    }
}

#[cfg(feature = "network-server")]
pub async fn serve<E, A, U>(
    listener: TcpListener,
    transport: Transport<E, A, U>,
) -> std::io::Result<()>
where
    E: RestrictedExecutor + Send + Sync + 'static,
    A: AuditSink + Send + Sync + 'static,
    U: Authenticator,
{
    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let service_transport = transport.clone();
        tokio::spawn(async move {
            let service = service_fn(move |request: Request<Incoming>| {
                let request_transport = service_transport.clone();
                async move { Ok::<_, Infallible>(request_transport.handle(request).await) }
            });
            let _ = http1::Builder::new().serve_connection(io, service).await;
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
