mod core_client;

use core_client::{CoreClient, CoreConversation, CoreHealth};
use serde::Serialize;
use std::sync::Mutex;
use std::time::Instant;
use sysinfo::{Disks, Networks, System};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TelemetrySnapshot {
    timestamp_ms: u128,
    cpu_usage: f32,
    memory_used: u64,
    memory_total: u64,
    memory_usage: f32,
    disk_used: u64,
    disk_total: u64,
    disk_usage: f32,
    network_rx_per_sec: u64,
    network_tx_per_sec: u64,
    uptime_seconds: u64,
    load_average: [f64; 3],
    hostname: String,
    kernel: String,
}

struct TelemetryCollector {
    system: System,
    networks: Networks,
    disks: Disks,
    last_refresh: Instant,
}

impl TelemetryCollector {
    fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        Self {
            system,
            networks: Networks::new_with_refreshed_list(),
            disks: Disks::new_with_refreshed_list(),
            last_refresh: Instant::now(),
        }
    }

    fn snapshot(&mut self) -> TelemetrySnapshot {
        let elapsed = self.last_refresh.elapsed().as_secs_f64().max(0.001);
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.networks.refresh(true);
        self.disks.refresh(true);
        self.last_refresh = Instant::now();

        let cpu_usage = self.system.global_cpu_usage();
        let memory_total = self.system.total_memory();
        let memory_used = self.system.used_memory();
        let memory_usage = percentage(memory_used, memory_total);

        let root_disk = self
            .disks
            .iter()
            .find(|disk| disk.mount_point().to_string_lossy() == "/")
            .or_else(|| self.disks.iter().next());
        let (disk_used, disk_total) = root_disk
            .map(|disk| {
                let total = disk.total_space();
                (total.saturating_sub(disk.available_space()), total)
            })
            .unwrap_or((0, 0));

        let rx_delta: u64 = self.networks.values().map(|data| data.received()).sum();
        let tx_delta: u64 = self.networks.values().map(|data| data.transmitted()).sum();
        let load = System::load_average();

        TelemetrySnapshot {
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            cpu_usage,
            memory_used,
            memory_total,
            memory_usage,
            disk_used,
            disk_total,
            disk_usage: percentage(disk_used, disk_total),
            network_rx_per_sec: (rx_delta as f64 / elapsed) as u64,
            network_tx_per_sec: (tx_delta as f64 / elapsed) as u64,
            uptime_seconds: System::uptime(),
            load_average: [load.one, load.five, load.fifteen],
            hostname: System::host_name().unwrap_or_else(|| "linux-node".into()),
            kernel: System::kernel_version().unwrap_or_else(|| "unknown".into()),
        }
    }
}

fn percentage(used: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        (used as f64 / total as f64 * 100.0).clamp(0.0, 100.0) as f32
    }
}

#[tauri::command]
fn get_system_telemetry(
    state: tauri::State<'_, Mutex<TelemetryCollector>>,
) -> Result<TelemetrySnapshot, String> {
    state
        .lock()
        .map_err(|_| "telemetry collector lock poisoned".to_string())
        .map(|mut collector| collector.snapshot())
}

#[tauri::command]
async fn get_core_health(state: tauri::State<'_, CoreClient>) -> Result<CoreHealth, String> {
    state.health().await.map_err(str::to_owned)
}

#[tauri::command]
async fn send_core_conversation(
    message: String,
    state: tauri::State<'_, CoreClient>,
) -> Result<CoreConversation, String> {
    state.conversation(message).await.map_err(str::to_owned)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let core_client = CoreClient::from_environment();
    tauri::Builder::default()
        .manage(Mutex::new(TelemetryCollector::new()))
        .manage(core_client)
        .invoke_handler(tauri::generate_handler![
            get_system_telemetry,
            get_core_health,
            send_core_conversation
        ])
        .run(tauri::generate_context!())
        .expect("error while running JARVIS");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentage_handles_normal_and_empty_values() {
        assert_eq!(percentage(50, 100), 50.0);
        assert_eq!(percentage(8, 0), 0.0);
    }

    #[test]
    fn snapshot_contains_real_host_data() {
        let snapshot = TelemetryCollector::new().snapshot();
        assert!(snapshot.memory_total > 0);
        assert!(!snapshot.hostname.is_empty());
        assert!((0.0..=100.0).contains(&snapshot.cpu_usage));
    }
}
