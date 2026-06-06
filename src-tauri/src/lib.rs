mod health;
mod listeners;
mod metrics;
mod models;
mod services;
mod thermals;
mod watchlist;

use std::time::{SystemTime, UNIX_EPOCH};

use models::{DashboardSnapshot, ServiceDetails, ServiceHealth, WatchedServicesSnapshot};

#[tauri::command]
fn snapshot() -> Result<DashboardSnapshot, String> {
    let mut warnings = Vec::new();
    let listeners = listeners::collect_listeners(&mut warnings);
    let temperatures = thermals::collect_temperatures(&mut warnings);
    let summary = metrics::collect_summary(listeners.len(), &mut warnings);

    let generated_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|error| format!("system clock error: {error}"))?;

    Ok(DashboardSnapshot {
        generated_at_unix_ms,
        summary,
        temperatures,
        listeners,
        warnings,
    })
}

#[tauri::command]
fn service_details(pid: u32, process_name: Option<String>) -> Result<ServiceDetails, String> {
    Ok(services::fetch_service_details(pid, process_name))
}

#[tauri::command]
fn service_health(
    protocol: String,
    local_address: String,
    port: u16,
) -> Result<ServiceHealth, String> {
    Ok(health::check_service_health(
        &protocol,
        &local_address,
        port,
    ))
}

pub fn watched_services_snapshot() -> WatchedServicesSnapshot {
    watchlist::collect_watched_services()
}

#[tauri::command]
fn watched_services() -> Result<WatchedServicesSnapshot, String> {
    Ok(watched_services_snapshot())
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            snapshot,
            service_details,
            service_health,
            watched_services
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| eprintln!("Sentinel runtime error: {error}"));
}
