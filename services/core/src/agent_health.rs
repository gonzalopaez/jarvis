use crate::{EventBus, EventType};
use reqwest::{Client, StatusCode, Url};
use serde_json::json;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const DEFAULT_AGENT_HEALTH_INTERVAL: Duration = Duration::from_secs(15);

/// A single service to poll for the HUD Agent Matrix. The service must expose
/// an unauthenticated GET route that returns a 2xx status when healthy.
#[derive(Clone)]
pub struct AgentHealthCheck {
    pub id: &'static str,
    pub label: &'static str,
    pub url: Url,
}

pub struct AgentHealthPoller {
    client: Client,
    checks: Vec<AgentHealthCheck>,
    interval: Duration,
}

impl AgentHealthPoller {
    pub fn new(checks: Vec<AgentHealthCheck>) -> Result<Self, &'static str> {
        Self::with_interval(checks, DEFAULT_AGENT_HEALTH_INTERVAL)
    }

    pub fn with_interval(
        checks: Vec<AgentHealthCheck>,
        interval: Duration,
    ) -> Result<Self, &'static str> {
        if checks.is_empty() {
            return Err("at least one agent health check is required");
        }
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(1))
            .timeout(Duration::from_secs(4))
            .build()
            .map_err(|_| "agent health client could not be created")?;
        Ok(Self {
            client,
            checks,
            interval,
        })
    }

    pub async fn run_until(
        self,
        events: EventBus,
        shutdown: impl std::future::Future<Output = ()>,
    ) {
        let mut interval = tokio::time::interval(self.interval);
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                _ = interval.tick() => self.collect_all(&events).await,
                () = &mut shutdown => break,
            }
        }
    }

    pub async fn collect_all(&self, events: &EventBus) {
        for check in &self.checks {
            self.collect_one(check, events).await;
        }
    }

    async fn collect_one(&self, check: &AgentHealthCheck, events: &EventBus) {
        let started = Instant::now();
        let status = self
            .client
            .get(check.url.clone())
            .send()
            .await
            .ok()
            .map(|response| response.status());
        let latency_ms: u64 = started.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
        let now_ms: u64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX);
        let (healthy, error) = classify(status);
        events.publish(
            EventType::AgentStatusChanged,
            None,
            json!({
                "id": check.id,
                "label": check.label,
                "status": if healthy { "healthy" } else { "unavailable" },
                "agent_status": if healthy { "realtime" } else { "offline" },
                "version": "adapter",
                "latency_ms": latency_ms,
                "last_seen_ms": now_ms,
                "error": error,
            }),
        );
    }
}

fn classify(status: Option<StatusCode>) -> (bool, Option<&'static str>) {
    match status {
        Some(status) if status.is_success() => (true, None),
        Some(_) => (false, Some("unhealthy")),
        None => (false, Some("unreachable")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_successful_status_is_healthy() {
        assert_eq!(classify(Some(StatusCode::OK)), (true, None));
    }

    #[test]
    fn a_non_success_status_is_unavailable_but_reachable() {
        assert_eq!(
            classify(Some(StatusCode::UNAUTHORIZED)),
            (false, Some("unhealthy"))
        );
        assert_eq!(
            classify(Some(StatusCode::SERVICE_UNAVAILABLE)),
            (false, Some("unhealthy"))
        );
    }

    #[test]
    fn a_missing_response_is_unreachable() {
        assert_eq!(classify(None), (false, Some("unreachable")));
    }

    #[test]
    fn constructing_a_poller_with_no_checks_is_rejected() {
        assert!(AgentHealthPoller::new(Vec::new()).is_err());
    }

    #[tokio::test]
    async fn a_real_health_response_is_published_as_agent_status_changed() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback listener");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buffer = [0_u8; 1024];
            let _ = socket.read(&mut buffer).await;
            let _ = socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .await;
        });

        let url: Url = format!("http://{addr}/v1/health").parse().expect("url");
        let poller = AgentHealthPoller::new(vec![AgentHealthCheck {
            id: "voice",
            label: "VOICE SERVICE",
            url,
        }])
        .expect("poller");

        let events = EventBus::new(8).expect("events");
        let mut receiver = events.subscribe();
        poller.collect_all(&events).await;

        let event = receiver.recv().await.expect("event");
        assert_eq!(event.event_type, "agent.status.changed");
        assert_eq!(event.payload["id"], "voice");
        assert_eq!(event.payload["status"], "healthy");
        assert_eq!(event.payload["agent_status"], "realtime");
        assert!(event.payload["error"].is_null());
    }

    #[tokio::test]
    async fn a_connection_failure_is_published_as_offline() {
        // Nothing is listening on this loopback port.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback listener");
        let addr = listener.local_addr().expect("local addr");
        drop(listener);

        let url: Url = format!("http://{addr}/v1/health").parse().expect("url");
        let poller = AgentHealthPoller::new(vec![AgentHealthCheck {
            id: "mcp",
            label: "MCP GATEWAY",
            url,
        }])
        .expect("poller");

        let events = EventBus::new(8).expect("events");
        let mut receiver = events.subscribe();
        poller.collect_all(&events).await;

        let event = receiver.recv().await.expect("event");
        assert_eq!(event.payload["id"], "mcp");
        assert_eq!(event.payload["status"], "unavailable");
        assert_eq!(event.payload["agent_status"], "offline");
        assert_eq!(event.payload["error"], "unreachable");
    }
}
