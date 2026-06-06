use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ServiceHealth {
    pub protocol: String,
    pub target: String,
    pub port: u16,
    pub check_kind: String,
    pub status: String,
    pub latency_ms: Option<u128>,
    pub message: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct WatchedServicesSnapshot {
    pub services: Vec<WatchedServiceStatus>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct WatchedServiceStatus {
    pub name: String,
    pub category: String,
    pub protocol: String,
    pub address: String,
    pub port: u16,
    pub expected: bool,
    pub listener: Option<Listener>,
    pub health: ServiceHealth,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DashboardSnapshot {
    pub generated_at_unix_ms: u128,
    pub summary: SystemSummary,
    pub temperatures: Vec<TemperatureReading>,
    pub listeners: Vec<Listener>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SystemSummary {
    pub cpu_usage_percent: Option<f32>,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub memory_available_bytes: u64,
    pub load_averages: [f32; 3],
    pub uptime_seconds: u64,
    pub listener_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TemperatureReading {
    pub label: String,
    pub celsius: f32,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Listener {
    pub protocol: String,
    pub local_address: String,
    pub port: u16,
    pub scope: String,
    pub port_kind: String,
    pub exposure_severity: String,
    pub process: Option<String>,
    pub pid: Option<u32>,
    pub unit: Option<String>,
    pub unit_scope: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "snake_case")]
pub struct SystemdUnitState {
    pub unit: String,
    pub description: String,
    pub load_state: String,
    pub active_state: String,
    pub sub_state: String,
    pub fragment_path: Option<String>,
    pub unit_file_state: Option<String>,
    pub main_pid: Option<u32>,
    pub is_user: bool,
}

#[derive(Debug, Clone)]
pub struct UnitRef {
    pub unit: String,
    pub is_user: bool,
    pub cgroup_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ServiceDetails {
    pub pid: u32,
    pub process_name: Option<String>,
    pub command_line: Option<String>,
    pub cgroup_path: Option<String>,
    pub resolved_unit: Option<String>,
    pub resolved_unit_scope: Option<String>,
    pub unit_state: Option<SystemdUnitState>,
    pub status_lines: Vec<String>,
    pub recent_logs: Vec<String>,
    pub warnings: Vec<String>,
}
