use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use crate::models::ServiceHealth;

const CONNECT_TIMEOUT: Duration = Duration::from_millis(1200);
const IO_TIMEOUT: Duration = Duration::from_millis(1200);

pub fn check_service_health(protocol: &str, local_address: &str, port: u16) -> ServiceHealth {
    let mut warnings = Vec::new();
    let target = sanitize_target(local_address);

    if protocol != "tcp" {
        return ServiceHealth {
            protocol: protocol.to_string(),
            target,
            port,
            check_kind: "socket".to_string(),
            status: "warning".to_string(),
            latency_ms: None,
            message: format!("active probing for protocol '{protocol}' is not implemented"),
            warnings,
        };
    }

    let socket = match resolve_socket_addr(&target, port) {
        Ok(socket) => socket,
        Err(message) => {
            warnings.push(message.clone());
            return ServiceHealth {
                protocol: protocol.to_string(),
                target,
                port,
                check_kind: "tcp_connect".to_string(),
                status: "danger".to_string(),
                latency_ms: None,
                message,
                warnings,
            };
        }
    };

    let started = Instant::now();
    let mut stream = match TcpStream::connect_timeout(&socket, CONNECT_TIMEOUT) {
        Ok(stream) => stream,
        Err(error) => {
            let message = format!("tcp connect failed: {error}");
            warnings.push(message.clone());
            return ServiceHealth {
                protocol: protocol.to_string(),
                target,
                port,
                check_kind: "tcp_connect".to_string(),
                status: "danger".to_string(),
                latency_ms: Some(started.elapsed().as_millis()),
                message,
                warnings,
            };
        }
    };

    let connect_latency_ms = started.elapsed().as_millis();
    if let Err(error) = stream.set_read_timeout(Some(IO_TIMEOUT)) {
        warnings.push(format!("set_read_timeout failed: {error}"));
    }
    if let Err(error) = stream.set_write_timeout(Some(IO_TIMEOUT)) {
        warnings.push(format!("set_write_timeout failed: {error}"));
    }

    if is_http_candidate(port) {
        match http_probe(&mut stream, &target) {
            Ok(status_line) => {
                let (status, message) = classify_http_status(&status_line);
                return ServiceHealth {
                    protocol: protocol.to_string(),
                    target,
                    port,
                    check_kind: "http_head".to_string(),
                    status: status.to_string(),
                    latency_ms: Some(connect_latency_ms),
                    message,
                    warnings,
                };
            }
            Err(error) => {
                warnings.push(format!("http probe failed: {error}"));
            }
        }
    }

    ServiceHealth {
        protocol: protocol.to_string(),
        target,
        port,
        check_kind: "tcp_connect".to_string(),
        status: "ok".to_string(),
        latency_ms: Some(connect_latency_ms),
        message: "tcp connect succeeded".to_string(),
        warnings,
    }
}

fn resolve_socket_addr(target: &str, port: u16) -> Result<SocketAddr, String> {
    let address = format!("{target}:{port}");
    let mut addrs = address
        .to_socket_addrs()
        .map_err(|error| format!("address resolution failed for {address}: {error}"))?;

    addrs
        .next()
        .ok_or_else(|| format!("address resolution returned no candidates for {address}"))
}

fn http_probe(stream: &mut TcpStream, target: &str) -> Result<String, String> {
    let request = format!(
        "HEAD / HTTP/1.1\r\nHost: {target}\r\nConnection: close\r\nUser-Agent: sentinel-health\r\n\r\n"
    );

    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("request write failed: {error}"))?;

    let mut buffer = [0_u8; 1024];
    let read = stream
        .read(&mut buffer)
        .map_err(|error| format!("response read failed: {error}"))?;

    if read == 0 {
        return Err("response read returned 0 bytes".to_string());
    }

    let response = String::from_utf8_lossy(&buffer[..read]);
    response
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "response status line missing".to_string())
}

fn classify_http_status(status_line: &str) -> (&'static str, String) {
    let maybe_code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|raw| raw.parse::<u16>().ok());

    match maybe_code {
        Some(code) if (200..400).contains(&code) => ("ok", format!("http probe ok: {status_line}")),
        Some(code) if (400..500).contains(&code) => (
            "warning",
            format!("http probe client response: {status_line}"),
        ),
        Some(_) => (
            "danger",
            format!("http probe error response: {status_line}"),
        ),
        None => (
            "warning",
            format!("http probe nonstandard status line: {status_line}"),
        ),
    }
}

fn is_http_candidate(port: u16) -> bool {
    matches!(
        port,
        80 | 443 | 8080 | 8443 | 3000 | 5173 | 11500 | 9000 | 9090
    )
}

fn sanitize_target(local_address: &str) -> String {
    match local_address {
        "*" | "0.0.0.0" | "::" => "127.0.0.1".to_string(),
        "::1" => "127.0.0.1".to_string(),
        value => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_http_status, sanitize_target};

    #[test]
    fn maps_wildcards_to_localhost() {
        assert_eq!(sanitize_target("0.0.0.0"), "127.0.0.1");
        assert_eq!(sanitize_target("*"), "127.0.0.1");
        assert_eq!(sanitize_target("::"), "127.0.0.1");
    }

    #[test]
    fn classifies_http_status_ranges() {
        let ok = classify_http_status("HTTP/1.1 200 OK");
        let warning = classify_http_status("HTTP/1.1 404 Not Found");
        let danger = classify_http_status("HTTP/1.1 503 Service Unavailable");

        assert_eq!(ok.0, "ok");
        assert_eq!(warning.0, "warning");
        assert_eq!(danger.0, "danger");
    }
}
