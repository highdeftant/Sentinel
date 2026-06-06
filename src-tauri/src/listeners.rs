use std::process::Command;

use crate::{models::Listener, services::resolve_unit_for_pid};

pub fn collect_listeners(warnings: &mut Vec<String>) -> Vec<Listener> {
    let output = match Command::new("ss").args(["-H", "-ltnup"]).output() {
        Ok(output) => output,
        Err(error) => {
            warnings.push(format!("listener collector could not execute ss: {error}"));
            return Vec::new();
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warnings.push(format!(
            "listener collector ss exited with {}: {}",
            output.status,
            stderr.trim()
        ));
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut listeners = Vec::new();

    for line in stdout.lines() {
        if let Some(mut listener) = parse_listener_line(line) {
            if let Some(unit_ref) = listener.pid.and_then(resolve_unit_for_pid) {
                listener.unit = Some(unit_ref.unit);
                listener.unit_scope = Some(if unit_ref.is_user {
                    "user".to_string()
                } else {
                    "system".to_string()
                });
            }

            listener.exposure_severity = classify_severity(
                &listener.scope,
                &listener.port_kind,
                listener.unit.is_some(),
            )
            .to_string();
            listeners.push(listener);
        }
    }

    listeners.sort_by(|left, right| {
        left.port
            .cmp(&right.port)
            .then_with(|| left.protocol.cmp(&right.protocol))
            .then_with(|| left.local_address.cmp(&right.local_address))
    });

    listeners
}

pub fn parse_listener_line(line: &str) -> Option<Listener> {
    let columns: Vec<&str> = line.split_whitespace().collect();
    if columns.len() < 5 {
        return None;
    }

    let protocol = columns.first()?.to_string();
    let local = *columns.get(4)?;
    let (local_address, port) = split_socket(local)?;
    let (process, pid) = parse_process_metadata(line);
    let scope = classify_scope(&local_address).to_string();
    let port_kind = classify_port_kind(&protocol, port).to_string();
    let exposure_severity = classify_severity(&scope, &port_kind, false).to_string();

    Some(Listener {
        protocol,
        local_address,
        port,
        scope,
        port_kind,
        exposure_severity,
        process,
        pid,
        unit: None,
        unit_scope: None,
    })
}

pub fn split_socket(raw: &str) -> Option<(String, u16)> {
    if raw == "*" {
        return None;
    }

    let (address, port_raw) = if raw.starts_with('[') {
        let end = raw.find(']')?;
        let address = &raw[1..end];
        let port_raw = raw.get(end + 2..)?;
        (address, port_raw)
    } else {
        let index = raw.rfind(':')?;
        (&raw[..index], &raw[index + 1..])
    };

    let port = port_raw.parse::<u16>().ok()?;
    let local_address = if address.is_empty() {
        "*".to_string()
    } else {
        address.to_string()
    };

    Some((local_address, port))
}

fn parse_process_metadata(line: &str) -> (Option<String>, Option<u32>) {
    let process = line
        .split("users:((\"")
        .nth(1)
        .and_then(|tail| tail.split("\"").next())
        .map(|value| value.to_string());

    let pid = line
        .split("pid=")
        .nth(1)
        .and_then(|tail| tail.split([',', ')']).next())
        .and_then(|raw| raw.parse::<u32>().ok());

    (process, pid)
}

pub fn classify_scope(address: &str) -> &'static str {
    match address {
        "127.0.0.1" | "::1" => "loopback",
        "0.0.0.0" | "*" | "::" => "all-ifaces",
        _ if address.starts_with("127.") => "loopback",
        _ if address.starts_with("fe80:") => "lan",
        _ if address.contains('%') => "lan",
        _ => "lan",
    }
}

pub fn classify_port_kind(protocol: &str, port: u16) -> &'static str {
    match protocol {
        "tcp" if is_standard_tcp_port(port) => "standard",
        "udp" if is_standard_udp_port(port) => "standard",
        _ => "nonstandard",
    }
}

pub fn classify_severity(scope: &str, port_kind: &str, managed: bool) -> &'static str {
    match (scope, port_kind, managed) {
        ("all-ifaces", "nonstandard", _) => "danger",
        ("all-ifaces", _, false) => "danger",
        ("lan", "nonstandard", _) => "warning",
        ("lan", _, false) => "warning",
        ("loopback", "nonstandard", false) => "warning",
        ("loopback", _, _) => "ok",
        (_, "nonstandard", _) => "warning",
        (_, _, false) => "warning",
        _ => "ok",
    }
}

fn is_standard_tcp_port(port: u16) -> bool {
    matches!(
        port,
        20 | 21
            | 22
            | 25
            | 53
            | 80
            | 110
            | 111
            | 135
            | 139
            | 143
            | 389
            | 443
            | 445
            | 465
            | 587
            | 631
            | 636
            | 873
            | 993
            | 995
            | 2049
            | 2377
            | 3306
            | 5432
            | 5900
            | 6379
            | 6443
            | 8080
            | 8443
            | 9000
            | 9090
    )
}

fn is_standard_udp_port(port: u16) -> bool {
    matches!(
        port,
        53 | 67 | 68 | 69 | 111 | 123 | 137 | 138 | 161 | 162 | 500 | 514 | 631
    )
}

#[cfg(test)]
mod tests {
    use super::{
        classify_port_kind, classify_scope, classify_severity, parse_listener_line, split_socket,
    };

    #[test]
    fn splits_ipv4_socket() {
        assert_eq!(
            split_socket("127.0.0.1:8645"),
            Some(("127.0.0.1".to_string(), 8645))
        );
    }

    #[test]
    fn splits_ipv6_socket() {
        assert_eq!(split_socket("[::]:8443"), Some(("::".to_string(), 8443)));
    }

    #[test]
    fn classifies_all_interfaces_scope() {
        assert_eq!(classify_scope("0.0.0.0"), "all-ifaces");
    }

    #[test]
    fn parses_listener_line_with_process_and_scope() {
        let line = "tcp LISTEN 0 128 127.0.0.1:8645 0.0.0.0:* users:((\"python\",pid=370995,fd=16)) uid:1000";
        let listener = parse_listener_line(line).expect("listener should parse");

        assert_eq!(listener.protocol, "tcp");
        assert_eq!(listener.local_address, "127.0.0.1");
        assert_eq!(listener.port, 8645);
        assert_eq!(listener.scope, "loopback");
        assert_eq!(listener.process.as_deref(), Some("python"));
        assert_eq!(listener.pid, Some(370995));
    }

    #[test]
    fn classifies_standard_port_kind() {
        assert_eq!(classify_port_kind("tcp", 22), "standard");
        assert_eq!(classify_port_kind("tcp", 443), "standard");
        assert_eq!(classify_port_kind("udp", 53), "standard");
    }

    #[test]
    fn classifies_nonstandard_port_kind() {
        assert_eq!(classify_port_kind("tcp", 11500), "nonstandard");
        assert_eq!(classify_port_kind("udp", 9999), "nonstandard");
    }

    #[test]
    fn marks_nonstandard_all_interfaces_as_danger() {
        assert_eq!(
            classify_severity("all-ifaces", "nonstandard", false),
            "danger"
        );
    }

    #[test]
    fn marks_standard_loopback_as_ok() {
        assert_eq!(classify_severity("loopback", "standard", true), "ok");
    }

    #[test]
    fn marks_unmanaged_lan_listener_as_warning() {
        assert_eq!(classify_severity("lan", "standard", false), "warning");
    }
}
