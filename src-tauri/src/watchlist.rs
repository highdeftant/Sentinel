use std::env;

use crate::{
    health::check_service_health,
    listeners::collect_listeners,
    models::{Listener, WatchedServiceStatus, WatchedServicesSnapshot},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchedService {
    pub name: String,
    pub category: String,
    pub protocol: String,
    pub address: String,
    pub port: u16,
    pub expected: bool,
}

pub fn collect_watched_services() -> WatchedServicesSnapshot {
    let mut warnings = Vec::new();
    let listeners = collect_listeners(&mut warnings);
    let watched = watched_services_from_env(&mut warnings);

    let services = watched
        .into_iter()
        .map(|service| {
            let listener = find_listener(&listeners, &service);
            let health = check_service_health(&service.protocol, &service.address, service.port);

            WatchedServiceStatus {
                name: service.name,
                category: service.category,
                protocol: service.protocol,
                address: service.address,
                port: service.port,
                expected: service.expected,
                listener,
                health,
            }
        })
        .collect();

    WatchedServicesSnapshot { services, warnings }
}

fn watched_services_from_env(warnings: &mut Vec<String>) -> Vec<WatchedService> {
    match env::var("SENTINEL_WATCH_PORTS") {
        Ok(raw) if !raw.trim().is_empty() => parse_watch_ports(&raw, warnings),
        Ok(_) | Err(_) => default_watch_services(),
    }
}

pub fn parse_watch_ports(raw: &str, warnings: &mut Vec<String>) -> Vec<WatchedService> {
    let mut services = Vec::new();

    for entry in raw
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        match parse_watch_entry(entry) {
            Some(service) => services.push(service),
            None => warnings.push(format!(
                "invalid SENTINEL_WATCH_PORTS entry '{entry}' expected name:protocol:address:port[:expected]"
            )),
        }
    }

    if services.is_empty() {
        warnings.push("SENTINEL_WATCH_PORTS produced no valid entries; using defaults".to_string());
        return default_watch_services();
    }

    services
}

fn parse_watch_entry(entry: &str) -> Option<WatchedService> {
    let parts = entry.split(':').collect::<Vec<_>>();
    if !(4..=5).contains(&parts.len()) {
        return None;
    }

    let name = parts.first()?.trim();
    let protocol = parts.get(1)?.trim().to_ascii_lowercase();
    let address = parts.get(2)?.trim();
    let port = parts.get(3)?.trim().parse::<u16>().ok()?;
    let expected = parts
        .get(4)
        .map(|value| parse_expected(value.trim()))
        .unwrap_or(Some(true))?;

    if name.is_empty() || address.is_empty() || !matches!(protocol.as_str(), "tcp" | "udp") {
        return None;
    }

    Some(WatchedService {
        name: name.to_string(),
        category: "custom".to_string(),
        protocol,
        address: address.to_string(),
        port,
        expected,
    })
}

fn parse_expected(raw: &str) -> Option<bool> {
    match raw.to_ascii_lowercase().as_str() {
        "true" | "yes" | "up" | "expected" | "1" => Some(true),
        "false" | "no" | "down" | "optional" | "0" => Some(false),
        _ => None,
    }
}

fn find_listener(listeners: &[Listener], service: &WatchedService) -> Option<Listener> {
    listeners
        .iter()
        .find(|listener| {
            listener.protocol == service.protocol
                && listener.port == service.port
                && address_matches(&listener.local_address, &service.address)
        })
        .cloned()
}

fn address_matches(listener_address: &str, watched_address: &str) -> bool {
    listener_address == watched_address
        || matches!(listener_address, "0.0.0.0" | "*" | "::")
        || matches!(watched_address, "127.0.0.1" | "localhost")
            && matches!(listener_address, "0.0.0.0" | "*" | "::")
}

fn default_watch_services() -> Vec<WatchedService> {
    vec![
        watched("sentinel-web-dashboard", "web", "tcp", "127.0.0.1", 11500),
        watched("yardline-web-dev", "web", "tcp", "127.0.0.1", 5173),
        watched("weatherbot-web-dashboard", "web", "tcp", "127.0.0.1", 5000),
        watched("honcho-api", "web", "tcp", "172.168.0.17", 8000),
        watched("nextcloud-http", "web", "tcp", "127.0.0.1", 8080),
        watched("nextcloud-https", "web", "tcp", "127.0.0.1", 8443),
        watched("kakusei-landing-static", "web", "tcp", "127.0.0.1", 8888),
        watched("dev-site-template-vite", "web", "tcp", "127.0.0.1", 1420),
        watched("camofox-browser-server", "web", "tcp", "127.0.0.1", 9377),
        watched("minio-s3-api", "infra", "tcp", "127.0.0.1", 9000),
        watched("minio-web-console", "infra", "tcp", "127.0.0.1", 9001),
        watched(
            "graffiti-postgres-docker",
            "infra",
            "tcp",
            "127.0.0.1",
            5433,
        ),
        watched("hermes-gateway-default", "hermes", "tcp", "127.0.0.1", 8644),
        watched("hermes-gateway-spoof", "hermes", "tcp", "127.0.0.1", 8645),
        watched("hermes-gateway-tracie", "hermes", "tcp", "127.0.0.1", 8646),
        watched("starbound-server", "game", "tcp", "127.0.0.1", 21025),
    ]
}

fn watched(name: &str, category: &str, protocol: &str, address: &str, port: u16) -> WatchedService {
    WatchedService {
        name: name.to_string(),
        category: category.to_string(),
        protocol: protocol.to_string(),
        address: address.to_string(),
        port,
        expected: true,
    }
}

#[cfg(test)]
mod tests {
    use super::{address_matches, parse_watch_ports};

    #[test]
    fn parses_watch_ports_env_format() {
        let mut warnings = Vec::new();
        let services = parse_watch_ports(
            "sentinel:tcp:127.0.0.1:11500,optional:tcp:127.0.0.1:9999:false",
            &mut warnings,
        );

        assert!(warnings.is_empty());
        assert_eq!(services.len(), 2);
        assert_eq!(services[0].name, "sentinel");
        assert_eq!(services[0].protocol, "tcp");
        assert_eq!(services[0].port, 11500);
        assert!(services[0].expected);
        assert!(!services[1].expected);
    }

    #[test]
    fn invalid_entries_emit_warning_and_fallback() {
        let mut warnings = Vec::new();
        let services = parse_watch_ports("bad-entry", &mut warnings);

        assert!(!services.is_empty());
        assert!(warnings.iter().any(|warning| warning.contains("invalid")));
    }

    #[test]
    fn wildcard_listener_matches_localhost_watch() {
        assert!(address_matches("0.0.0.0", "127.0.0.1"));
        assert!(address_matches("*", "localhost"));
        assert!(address_matches("::", "127.0.0.1"));
    }
}
