use crate::{EventBus, EventType};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

#[derive(Clone)]
pub struct WazuhSecurityPoller {
    client: Client,
    url: Url,
    token: String,
}

#[derive(Debug, Deserialize)]
struct RelayResponse {
    alerts: Vec<Alert>,
    metrics: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct Alert {
    id: String,
    host: String,
    timestamp_ms: u64,
    severity: String,
    title: String,
    description: String,
}

impl WazuhSecurityPoller {
    pub fn new(url: Url, token: String) -> Result<Self, &'static str> {
        if url.scheme() != "http" || url.host_str().is_none() || token.len() < 32 {
            return Err("invalid Wazuh relay configuration");
        }
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(1))
            .timeout(Duration::from_secs(4))
            .build()
            .map_err(|_| "Wazuh relay client could not be created")?;
        Ok(Self { client, url, token })
    }

    pub async fn run_until(self, events: EventBus, shutdown: impl std::future::Future<Output = ()>) {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                _ = interval.tick() => self.collect(&events).await,
                () = &mut shutdown => break,
            }
        }
    }

    async fn collect(&self, events: &EventBus) {
        let response = self.client.get(self.url.clone()).bearer_auth(&self.token).send().await;
        let Ok(response) = response else {
            events.publish(EventType::TelemetrySourceStatus, None, json!({"source":"wazuh","status":"unavailable"}));
            return;
        };
        let Ok(body) = response.json::<RelayResponse>().await else {
            events.publish(EventType::TelemetrySourceStatus, None, json!({"source":"wazuh","status":"rejected"}));
            return;
        };
        events.publish(EventType::TelemetrySourceStatus, None, json!({"source":"wazuh","status":"healthy"}));
        events.publish(EventType::SecurityTelemetryUpdated, None, body.metrics);
        for alert in body.alerts.into_iter().take(20) {
            events.publish(EventType::SecurityAlert, None, json!({
                "id": alert.id, "host": alert.host, "timestamp_ms": alert.timestamp_ms,
                "severity": alert.severity, "title": alert.title,
                "description": alert.description
            }));
        }
    }
}
