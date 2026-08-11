use crate::{EventBus, EventType};
use serde::Serialize;
use serde_json::json;
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

#[cfg(feature = "network-server")]
use serde::Deserialize;
#[cfg(feature = "network-server")]
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_TELEMETRY_INTERVAL: Duration = Duration::from_secs(10);
pub const DEFAULT_TELEMETRY_ADAPTER_TIMEOUT: Duration = Duration::from_secs(5);
pub const MIN_TELEMETRY_INTERVAL: Duration = Duration::from_secs(2);
pub const MAX_TELEMETRY_INTERVAL: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TelemetrySource {
    Prometheus,
    Wazuh,
    Jarvis,
}

impl TelemetrySource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Prometheus => "prometheus",
            Self::Wazuh => "wazuh",
            Self::Jarvis => "jarvis",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TemperatureReading {
    pub sensor: String,
    pub celsius: f32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct OperationalTelemetry {
    pub timestamp_ms: u64,
    pub host: String,
    pub kernel: String,
    pub cpu_usage_percent: f32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub load_average: [f64; 3],
    pub filesystem_used_bytes: u64,
    pub filesystem_total_bytes: u64,
    pub disk_read_bytes_per_second: u64,
    pub disk_write_bytes_per_second: u64,
    pub network_receive_bytes_per_second: u64,
    pub network_transmit_bytes_per_second: u64,
    pub uptime_seconds: u64,
    pub temperatures: Vec<TemperatureReading>,
}

impl OperationalTelemetry {
    pub fn validate(&self) -> Result<(), TelemetryValidationError> {
        if self.timestamp_ms == 0
            || !valid_name(&self.host)
            || !valid_name(&self.kernel)
            || !self.cpu_usage_percent.is_finite()
            || !(0.0..=100.0).contains(&self.cpu_usage_percent)
            || self.memory_total_bytes == 0
            || self.memory_used_bytes > self.memory_total_bytes
            || self.filesystem_total_bytes == 0
            || self.filesystem_used_bytes > self.filesystem_total_bytes
            || self
                .load_average
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
            || self.temperatures.len() > 64
            || self.temperatures.iter().any(|reading| {
                !valid_name(&reading.sensor)
                    || !reading.celsius.is_finite()
                    || !(-50.0..=200.0).contains(&reading.celsius)
            })
        {
            return Err(TelemetryValidationError);
        }
        Ok(())
    }
}

pub trait TelemetryAdapter: Send + Sync + 'static {
    fn source(&self) -> TelemetrySource;
    fn collect(
        &self,
    ) -> Pin<
        Box<dyn Future<Output = Result<OperationalTelemetry, TelemetryAdapterError>> + Send + '_>,
    >;
}

pub struct UnavailableTelemetryAdapter {
    source: TelemetrySource,
}

impl UnavailableTelemetryAdapter {
    pub fn new(source: TelemetrySource) -> Self {
        Self { source }
    }
}

impl TelemetryAdapter for UnavailableTelemetryAdapter {
    fn source(&self) -> TelemetrySource {
        self.source
    }

    fn collect(
        &self,
    ) -> Pin<
        Box<dyn Future<Output = Result<OperationalTelemetry, TelemetryAdapterError>> + Send + '_>,
    > {
        Box::pin(async { Err(TelemetryAdapterError::Unavailable) })
    }
}

#[cfg(feature = "network-server")]
#[derive(Clone)]
pub struct PrometheusTelemetryAdapter {
    client: reqwest::Client,
    query_url: reqwest::Url,
    instance: String,
}

#[cfg(feature = "network-server")]
impl PrometheusTelemetryAdapter {
    pub fn new(base_url: reqwest::Url, instance: String) -> Result<Self, TelemetryAdapterError> {
        if base_url.scheme() != "http"
            || base_url.host_str().is_none()
            || !base_url.username().is_empty()
            || base_url.password().is_some()
            || !valid_name(&instance)
        {
            return Err(TelemetryAdapterError::Rejected);
        }
        let query_url = base_url
            .join("api/v1/query")
            .map_err(|_| TelemetryAdapterError::Rejected)?;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(1))
            .timeout(Duration::from_secs(3))
            .build()
            .map_err(|_| TelemetryAdapterError::Rejected)?;
        Ok(Self {
            client,
            query_url,
            instance,
        })
    }

    async fn scalar(&self, expression: String) -> Result<f64, TelemetryAdapterError> {
        let response = self
            .client
            .get(self.query_url.clone())
            .query(&[("query", expression)])
            .send()
            .await
            .map_err(map_request_error)?;
        if !response.status().is_success() {
            return Err(TelemetryAdapterError::Rejected);
        }
        let body: PrometheusResponse = response
            .json()
            .await
            .map_err(|_| TelemetryAdapterError::Rejected)?;
        body.scalar()
    }

    async fn down_targets(&self) -> Result<Vec<(String, String)>, TelemetryAdapterError> {
        let response = self
            .client
            .get(self.query_url.clone())
            .query(&[(
                "query",
                "up == 0 or jarvis_proxmox_guest_up == 0 or jarvis_proxmox_service_up == 0",
            )])
            .send()
            .await
            .map_err(map_request_error)?;
        if !response.status().is_success() {
            return Err(TelemetryAdapterError::Rejected);
        }
        let body: PrometheusResponse = response
            .json()
            .await
            .map_err(|_| TelemetryAdapterError::Rejected)?;
        if body.status != "success" {
            return Err(TelemetryAdapterError::Unavailable);
        }
        Ok(body
            .data
            .result
            .into_iter()
            .filter_map(|sample| {
                let value = sample.value.1.parse::<f64>().ok()?;
                if value != 0.0 {
                    return None;
                }
                let component = sample
                    .metric
                    .get("name")
                    .or_else(|| sample.metric.get("component"))
                    .cloned()
                    .unwrap_or_else(|| "servicio desconocido".into());
                let instance = sample
                    .metric
                    .get("instance")
                    .cloned()
                    .unwrap_or_else(|| "instancia desconocida".into());
                Some((component, instance))
            })
            .collect())
    }

    fn selector(&self) -> String {
        format!("instance=\"{}\"", self.instance)
    }
}

#[cfg(feature = "network-server")]
impl TelemetryAdapter for PrometheusTelemetryAdapter {
    fn source(&self) -> TelemetrySource {
        TelemetrySource::Prometheus
    }

    fn collect(
        &self,
    ) -> Pin<
        Box<dyn Future<Output = Result<OperationalTelemetry, TelemetryAdapterError>> + Send + '_>,
    > {
        Box::pin(async move {
            let selector = self.selector();
            let cpu = self.scalar(format!(
                "100 - avg(rate(node_cpu_seconds_total{{{selector},mode=\"idle\"}}[2m])) * 100"
            ));
            let memory_total = self.scalar(format!("node_memory_MemTotal_bytes{{{selector}}}"));
            let memory_available =
                self.scalar(format!("node_memory_MemAvailable_bytes{{{selector}}}"));
            let load_1 = self.scalar(format!("node_load1{{{selector}}}"));
            let load_5 = self.scalar(format!("node_load5{{{selector}}}"));
            let load_15 = self.scalar(format!("node_load15{{{selector}}}"));
            let filesystem_total = self.scalar(format!(
                "node_filesystem_size_bytes{{{selector},mountpoint=\"/\"}}"
            ));
            let filesystem_available = self.scalar(format!(
                "node_filesystem_avail_bytes{{{selector},mountpoint=\"/\"}}"
            ));
            let disk_read = self.scalar(format!(
                "sum(rate(node_disk_read_bytes_total{{{selector}}}[2m]))"
            ));
            let disk_write = self.scalar(format!(
                "sum(rate(node_disk_written_bytes_total{{{selector}}}[2m]))"
            ));
            let network_receive = self.scalar(format!(
                "sum(rate(node_network_receive_bytes_total{{{selector},device!=\"lo\"}}[2m]))"
            ));
            let network_transmit = self.scalar(format!(
                "sum(rate(node_network_transmit_bytes_total{{{selector},device!=\"lo\"}}[2m]))"
            ));
            let uptime = self.scalar(format!("time() - node_boot_time_seconds{{{selector}}}"));
            let (
                cpu,
                memory_total,
                memory_available,
                load_1,
                load_5,
                load_15,
                filesystem_total,
                filesystem_available,
                disk_read,
                disk_write,
                network_receive,
                network_transmit,
                uptime,
            ) = tokio::join!(
                cpu,
                memory_total,
                memory_available,
                load_1,
                load_5,
                load_15,
                filesystem_total,
                filesystem_available,
                disk_read,
                disk_write,
                network_receive,
                network_transmit,
                uptime,
            );

            let total_memory = bounded_u64(memory_total?)?;
            let available_memory = bounded_u64(memory_available?)?;
            let total_filesystem = bounded_u64(filesystem_total?)?;
            let available_filesystem = bounded_u64(filesystem_available?)?;
            let timestamp_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| TelemetryAdapterError::Rejected)?
                .as_millis()
                .try_into()
                .map_err(|_| TelemetryAdapterError::Rejected)?;

            Ok(OperationalTelemetry {
                timestamp_ms,
                host: "server-central".into(),
                kernel: "proxmox-linux".into(),
                cpu_usage_percent: cpu?.clamp(0.0, 100.0) as f32,
                memory_used_bytes: total_memory.saturating_sub(available_memory),
                memory_total_bytes: total_memory,
                load_average: [load_1?, load_5?, load_15?],
                filesystem_used_bytes: total_filesystem.saturating_sub(available_filesystem),
                filesystem_total_bytes: total_filesystem,
                disk_read_bytes_per_second: bounded_u64(disk_read?)?,
                disk_write_bytes_per_second: bounded_u64(disk_write?)?,
                network_receive_bytes_per_second: bounded_u64(network_receive?)?,
                network_transmit_bytes_per_second: bounded_u64(network_transmit?)?,
                uptime_seconds: bounded_u64(uptime?)?,
                temperatures: Vec::new(),
            })
        })
    }
}

#[cfg(feature = "network-server")]
#[derive(Deserialize)]
struct PrometheusResponse {
    status: String,
    data: PrometheusData,
}

#[cfg(feature = "network-server")]
#[derive(Deserialize)]
struct PrometheusData {
    result: Vec<PrometheusSample>,
}

#[cfg(feature = "network-server")]
#[derive(Deserialize)]
struct PrometheusSample {
    #[serde(default)]
    metric: std::collections::HashMap<String, String>,
    value: (f64, String),
}

#[cfg(feature = "network-server")]
impl PrometheusResponse {
    fn scalar(self) -> Result<f64, TelemetryAdapterError> {
        if self.status != "success" || self.data.result.len() != 1 {
            return Err(TelemetryAdapterError::Unavailable);
        }
        let value = self.data.result[0]
            .value
            .1
            .parse::<f64>()
            .map_err(|_| TelemetryAdapterError::Rejected)?;
        if !value.is_finite() || value < 0.0 {
            return Err(TelemetryAdapterError::Rejected);
        }
        Ok(value)
    }
}

#[cfg(feature = "network-server")]
pub async fn run_prometheus_availability_until(
    adapter: Arc<PrometheusTelemetryAdapter>,
    events: EventBus,
    shutdown: impl Future<Output = ()>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    tokio::pin!(shutdown);
    // Targets already reported DOWN, so we only publish on the up->down edge
    // instead of re-emitting an identical alert on every 10s scrape (which would
    // flood the bounded event history and every WebSocket client).
    let mut down_targets: std::collections::HashSet<String> = std::collections::HashSet::new();
    loop {
        tokio::select! {
            _ = interval.tick() => {
                if let Ok(targets) = adapter.down_targets().await {
                    let mut current = std::collections::HashSet::with_capacity(targets.len());
                    for (component, instance) in targets {
                        let key = format!("{}-{}", component, instance);
                        if !down_targets.contains(&key) {
                            events.publish(EventType::SecurityAlert, None, json!({
                                "id": format!("prometheus-up-{key}"),
                                "host": component,
                                "timestamp_ms": chrono_like_now_ms(),
                                "severity": "high",
                                "title": "Servicio caído según Prometheus",
                                "description": format!("Prometheus reporta DOWN: {}", component),
                            }));
                        }
                        current.insert(key);
                    }
                    // Targets absent from this successful scrape have recovered;
                    // forgetting them lets a future outage alert again.
                    down_targets = current;
                }
            }
            () = &mut shutdown => break,
        }
    }
}

#[cfg(feature = "network-server")]
fn chrono_like_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(feature = "network-server")]
fn map_request_error(error: reqwest::Error) -> TelemetryAdapterError {
    if error.is_timeout() {
        TelemetryAdapterError::DeadlineExceeded
    } else {
        TelemetryAdapterError::Unavailable
    }
}

#[cfg(feature = "network-server")]
fn bounded_u64(value: f64) -> Result<u64, TelemetryAdapterError> {
    if !value.is_finite() || !(0.0..=u64::MAX as f64).contains(&value) {
        return Err(TelemetryAdapterError::Rejected);
    }
    Ok(value.round() as u64)
}

pub struct TelemetryService {
    adapters: Vec<Arc<dyn TelemetryAdapter>>,
    events: EventBus,
    interval: Duration,
}

impl TelemetryService {
    pub fn new(
        adapters: Vec<Arc<dyn TelemetryAdapter>>,
        events: EventBus,
        interval: Duration,
    ) -> Result<Self, TelemetryServiceConfigError> {
        if adapters.is_empty()
            || adapters.len() > 8
            || interval < MIN_TELEMETRY_INTERVAL
            || interval > MAX_TELEMETRY_INTERVAL
        {
            return Err(TelemetryServiceConfigError);
        }
        Ok(Self {
            adapters,
            events,
            interval,
        })
    }

    pub async fn collect_once(&self) {
        for adapter in &self.adapters {
            let source = adapter.source();
            match tokio::time::timeout(DEFAULT_TELEMETRY_ADAPTER_TIMEOUT, adapter.collect()).await {
                Ok(Ok(sample)) if sample.validate().is_ok() => {
                    self.events.publish(
                        EventType::TelemetrySourceStatus,
                        None,
                        json!({ "source": source.as_str(), "status": "healthy" }),
                    );
                    self.events.publish(
                        EventType::TelemetrySnapshot,
                        None,
                        serde_json::to_value(sample).unwrap_or_else(|_| json!({})),
                    );
                }
                Ok(Ok(_)) => {
                    self.publish_failure(source, "invalid_data");
                }
                Ok(Err(TelemetryAdapterError::Unavailable)) => {
                    self.publish_failure(source, "unavailable");
                }
                Ok(Err(TelemetryAdapterError::DeadlineExceeded)) | Err(_) => {
                    self.publish_failure(source, "deadline_exceeded");
                }
                Ok(Err(TelemetryAdapterError::Rejected)) => {
                    self.publish_failure(source, "rejected");
                }
            }
        }
    }

    pub async fn run_until<S>(self, shutdown: S)
    where
        S: Future<Output = ()>,
    {
        let mut interval = tokio::time::interval(self.interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                _ = interval.tick() => self.collect_once().await,
                () = &mut shutdown => break,
            }
        }
    }

    fn publish_failure(&self, source: TelemetrySource, status: &'static str) {
        self.events.publish(
            EventType::TelemetrySourceStatus,
            None,
            json!({ "source": source.as_str(), "status": status }),
        );
    }
}

fn valid_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryAdapterError {
    Unavailable,
    DeadlineExceeded,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelemetryValidationError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelemetryServiceConfigError;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn rejects_invalid_normalized_metrics() {
        let mut sample = fixture();
        sample.cpu_usage_percent = 101.0;
        assert!(sample.validate().is_err());
        sample = fixture();
        sample.memory_used_bytes = sample.memory_total_bytes + 1;
        assert!(sample.validate().is_err());
    }

    #[cfg(feature = "network-server")]
    #[test]
    fn prometheus_adapter_rejects_credentials_and_unsafe_instance_labels() {
        let credentialed = "http://user:secret@192.168.1.24:9090/"
            .parse()
            .expect("url");
        assert!(PrometheusTelemetryAdapter::new(credentialed, "core:9100".into()).is_err());
        let private_url = "http://192.168.1.24:9090/".parse().expect("url");
        assert!(
            PrometheusTelemetryAdapter::new(private_url, "core\"} or vector(1)".into()).is_err()
        );
    }

    #[cfg(feature = "network-server")]
    #[test]
    fn prometheus_scalar_requires_one_finite_non_negative_sample() {
        let valid: PrometheusResponse = serde_json::from_value(json!({
            "status": "success",
            "data": { "result": [{ "value": [1_700_000_000.0, "42.5"] }] }
        }))
        .expect("response");
        assert_eq!(valid.scalar().expect("scalar"), 42.5);

        let empty: PrometheusResponse = serde_json::from_value(json!({
            "status": "success",
            "data": { "result": [] }
        }))
        .expect("response");
        assert_eq!(empty.scalar(), Err(TelemetryAdapterError::Unavailable));
    }

    #[tokio::test]
    async fn unavailable_adapter_publishes_status_without_fake_metrics() {
        let events = EventBus::new(8).expect("events");
        let mut receiver = events.subscribe();
        let service = TelemetryService::new(
            vec![Arc::new(UnavailableTelemetryAdapter::new(
                TelemetrySource::Prometheus,
            ))],
            events,
            DEFAULT_TELEMETRY_INTERVAL,
        )
        .expect("service");
        service.collect_once().await;
        let event = receiver.recv().await.expect("status");
        assert_eq!(event.event_type, "telemetry.source.status");
        assert_eq!(event.payload["status"], "unavailable");
        assert!(receiver.try_recv().is_err());
    }

    fn fixture() -> OperationalTelemetry {
        OperationalTelemetry {
            timestamp_ms: 1,
            host: "server-1".into(),
            kernel: "linux-6.0".into(),
            cpu_usage_percent: 10.0,
            memory_used_bytes: 1,
            memory_total_bytes: 2,
            load_average: [0.1, 0.2, 0.3],
            filesystem_used_bytes: 1,
            filesystem_total_bytes: 2,
            disk_read_bytes_per_second: 1,
            disk_write_bytes_per_second: 1,
            network_receive_bytes_per_second: 1,
            network_transmit_bytes_per_second: 1,
            uptime_seconds: 1,
            temperatures: vec![],
        }
    }
}
