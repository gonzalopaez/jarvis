use crate::task_outbox::{WAZUH_APPROACH, WAZUH_TASK_TYPE};
use crate::{
    CommittedTaskOutcomeRecord, CoreOutboxStore, DurableAuditEvent, EventBus, EventType,
    TaskOutcomeProvenance,
};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_WAZUH_READ_BYTES: usize = 256 * 1024;
const MAX_WAZUH_ALERTS: usize = 20;

#[derive(Clone)]
pub struct WazuhSecurityPoller {
    client: Client,
    url: Url,
    token: String,
}

#[derive(Debug, Deserialize)]
struct RelayResponse {
    alerts: Vec<Alert>,
    #[serde(default)]
    metrics: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct Alert {
    id: Option<String>,
    #[serde(default)]
    host: Option<String>,
    timestamp_ms: Option<u64>,
    severity: String,
    title: Option<String>,
    description: Option<String>,
    #[serde(default)]
    source_ip: Option<String>,
    #[serde(default)]
    wazuh: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WazuhReadFilter {
    pub host: Option<String>,
    pub severity: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedWazuhRead {
    pub alert_count: usize,
    pub critical_count: usize,
    pub high_count: usize,
    pub medium_count: usize,
    pub low_count: usize,
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

    pub async fn run_until(
        self,
        events: EventBus,
        shutdown: impl std::future::Future<Output = ()>,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                _ = interval.tick() => self.collect(&events).await,
                () = &mut shutdown => break,
            }
        }
    }

    /// Executes the real Tier 1 `wazuh.alerts.read` path. Only typed counts are
    /// returned; alert prose is deliberately excluded from the learning input.
    pub async fn verified_read(
        &self,
        filter: &WazuhReadFilter,
    ) -> Result<VerifiedWazuhRead, &'static str> {
        validate_filter(filter)?;
        let response = self
            .client
            .get(self.url.clone())
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|_| "Wazuh relay unavailable")?;
        if !response.status().is_success() {
            return Err("Wazuh relay rejected read");
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_| "invalid Wazuh response")?;
        if bytes.len() > MAX_WAZUH_READ_BYTES {
            return Err("Wazuh response exceeded limit");
        }
        let body: RelayResponse =
            serde_json::from_slice(&bytes).map_err(|_| "invalid Wazuh response")?;
        if body.alerts.len() > MAX_WAZUH_ALERTS {
            return Err("Wazuh response contained too many alerts");
        }
        let mut result = VerifiedWazuhRead {
            alert_count: 0,
            critical_count: 0,
            high_count: 0,
            medium_count: 0,
            low_count: 0,
        };
        for alert in body.alerts {
            validate_alert(&alert)?;
            if filter
                .host
                .as_deref()
                .is_some_and(|host| alert.host.as_deref() != Some(host))
                || filter
                    .severity
                    .as_deref()
                    .is_some_and(|severity| severity != alert.severity)
            {
                continue;
            }
            if result.alert_count == filter.limit {
                break;
            }
            result.alert_count += 1;
            match alert.severity.as_str() {
                "critical" => result.critical_count += 1,
                "high" => result.high_count += 1,
                "medium" => result.medium_count += 1,
                "low" => result.low_count += 1,
                _ => return Err("invalid Wazuh severity"),
            }
        }
        Ok(result)
    }

    pub async fn verified_read_and_commit(
        &self,
        store: &CoreOutboxStore,
        request_id: &str,
        subject: &str,
        filter: &WazuhReadFilter,
    ) -> Result<CommittedTaskOutcomeRecord, &'static str> {
        if !valid_identity(request_id) || !valid_identity(subject) {
            return Err("invalid Wazuh task identity");
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "invalid system time")?;
        let audit_id = opaque_id("audit", request_id, now.as_nanos());
        let event_id = opaque_id("event", &audit_id, now.as_nanos());
        let provenance = TaskOutcomeProvenance {
            kind: "validated_read_adapter".into(),
            adapter: "wazuh-relay-http".into(),
            response_schema: "wazuh-normalized.v1".into(),
        };
        let result = match self.verified_read(filter).await {
            Ok(result) => result,
            Err(error) => {
                let failure = DurableAuditEvent {
                    audit_id,
                    request_id: request_id.into(),
                    subject: subject.into(),
                    capability: "wazuh.alerts.read".into(),
                    capability_tier: 1,
                    outcome: "verification_failed",
                    provenance,
                };
                store
                    .commit_failure(&failure)
                    .await
                    .map_err(|_| "Core audit unavailable")?;
                return Err(error);
            }
        };
        let record = CommittedTaskOutcomeRecord {
            schema_version: "task_outcome.verified.v1".into(),
            source_event_id: event_id,
            source_audit_id: audit_id.clone(),
            source_request_id: request_id.into(),
            subject: subject.into(),
            task_type: WAZUH_TASK_TYPE.into(),
            context_summary: format!(
                "Alert counts: total={}, critical={}, high={}, medium={}, low={}",
                result.alert_count,
                result.critical_count,
                result.high_count,
                result.medium_count,
                result.low_count
            ),
            approach: WAZUH_APPROACH.into(),
            capability: "wazuh.alerts.read".into(),
            capability_tier: 1,
            executor_verified: true,
            human_authorization_audit_id: None,
            provenance: provenance.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            committed_at_epoch_seconds: now.as_secs(),
        };
        let audit = DurableAuditEvent {
            audit_id,
            request_id: request_id.into(),
            subject: subject.into(),
            capability: "wazuh.alerts.read".into(),
            capability_tier: 1,
            outcome: "verified",
            provenance,
        };
        store
            .commit_verified(&audit, &record)
            .await
            .map_err(|_| "Core outbox unavailable")?;
        Ok(record)
    }

    async fn collect(&self, events: &EventBus) {
        let response = self
            .client
            .get(self.url.clone())
            .bearer_auth(&self.token)
            .send()
            .await;
        let Ok(response) = response else {
            events.publish(
                EventType::TelemetrySourceStatus,
                None,
                json!({"source":"wazuh","status":"unavailable"}),
            );
            return;
        };
        if !response.status().is_success() {
            events.publish(
                EventType::TelemetrySourceStatus,
                None,
                json!({"source":"wazuh","status":"rejected"}),
            );
            return;
        }
        let Ok(body) = response.json::<RelayResponse>().await else {
            events.publish(
                EventType::TelemetrySourceStatus,
                None,
                json!({"source":"wazuh","status":"rejected"}),
            );
            return;
        };
        events.publish(
            EventType::TelemetrySourceStatus,
            None,
            json!({"source":"wazuh","status":"healthy"}),
        );
        events.publish(EventType::SecurityTelemetryUpdated, None, body.metrics);
        for alert in body.alerts.into_iter().take(20) {
            events.publish(
                EventType::SecurityAlert,
                None,
                json!({
                    "id": alert.id, "host": alert.host, "timestamp_ms": alert.timestamp_ms,
                    "severity": alert.severity, "title": alert.title,
                    "description": alert.description, "source_ip": alert.source_ip,
                    "wazuh": alert.wazuh
                }),
            );
        }
    }
}

fn validate_filter(filter: &WazuhReadFilter) -> Result<(), &'static str> {
    if filter.limit == 0
        || filter.limit > MAX_WAZUH_ALERTS
        || filter
            .host
            .as_deref()
            .is_some_and(|value| !valid_identity(value))
        || filter
            .severity
            .as_deref()
            .is_some_and(|value| !matches!(value, "critical" | "high" | "medium" | "low"))
    {
        return Err("invalid Wazuh read filter");
    }
    Ok(())
}

fn validate_alert(alert: &Alert) -> Result<(), &'static str> {
    if alert
        .id
        .as_deref()
        .is_none_or(|value| !valid_identity(value))
        || alert.timestamp_ms.is_none_or(|value| value == 0)
        || alert
            .host
            .as_deref()
            .is_some_and(|value| !valid_identity(value))
        || !matches!(
            alert.severity.as_str(),
            "critical" | "high" | "medium" | "low"
        )
    {
        return Err("invalid normalized Wazuh alert");
    }
    Ok(())
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'@')
        })
}

fn opaque_id(prefix: &str, seed: &str, nonce: u128) -> String {
    let digest = hex::encode(Sha256::digest(format!("{seed}:{nonce}").as_bytes()));
    format!("{prefix}-{}", &digest[..32])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[tokio::test]
    async fn real_wazuh_read_path_validates_and_returns_only_typed_counts() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let length = stream.read(&mut request).unwrap();
            assert!(String::from_utf8_lossy(&request[..length])
                .to_ascii_lowercase()
                .contains("authorization: bearer"));
            let body = r#"{"alerts":[{"id":"wazuh-1","host":"node-1","timestamp_ms":1789128000000,"severity":"critical","title":"ignored model input","description":"ignored model input","source_ip":"192.0.2.1","wazuh":{}}],"metrics":{}}"#;
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        });
        let adapter = WazuhSecurityPoller::new(
            format!("http://{address}/").parse().unwrap(),
            "w".repeat(32),
        )
        .unwrap();
        let result = adapter
            .verified_read(&WazuhReadFilter {
                host: Some("node-1".into()),
                severity: Some("critical".into()),
                limit: 20,
            })
            .await
            .unwrap();
        server.join().unwrap();
        assert_eq!(result.alert_count, 1);
        assert_eq!(result.critical_count, 1);
    }

    #[test]
    fn wazuh_read_filters_are_bounded() {
        assert!(validate_filter(&WazuhReadFilter {
            host: None,
            severity: None,
            limit: 21
        })
        .is_err());
        assert!(validate_filter(&WazuhReadFilter {
            host: None,
            severity: Some("fatal".into()),
            limit: 1
        })
        .is_err());
    }
}
