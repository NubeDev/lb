//! `host.net.reach` — bounded TCP reachability probe.

use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use lb_mcp::ToolError;
use serde::Serialize;
use serde_json::Value;

pub const HOST_NET_REACH_DEFAULT_TIMEOUT_MS: u64 = 2_000;
pub const HOST_NET_REACH_MAX_TIMEOUT_MS: u64 = 5_000;
const MAX_RESOLVED_ADDRS: usize = 8;

#[derive(Debug, Clone, Serialize)]
pub struct HostNetReach {
    pub host: String,
    pub port: u16,
    pub reachable: bool,
    pub latency_ms: Option<u64>,
    pub timeout_ms: u64,
    pub error: Option<String>,
}

pub async fn host_net_reach(input: &Value) -> Result<HostNetReach, ToolError> {
    let host = input
        .get("host")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ToolError::BadInput("missing arg: host".into()))?
        .to_string();
    let port = parse_port(input)?;
    let timeout_ms = capped_timeout(input);

    let task_host = host.clone();
    let task = tokio::task::spawn_blocking(move || probe(task_host, port, timeout_ms));
    match tokio::time::timeout(Duration::from_millis(timeout_ms), task).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => Err(ToolError::Extension(e.to_string())),
        Err(_) => Ok(HostNetReach {
            host,
            port,
            reachable: false,
            latency_ms: None,
            timeout_ms,
            error: Some("timeout".to_string()),
        }),
    }
}

fn parse_port(input: &Value) -> Result<u16, ToolError> {
    let raw = input
        .get("port")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| ToolError::BadInput("missing arg: port".into()))?;
    let port =
        u16::try_from(raw).map_err(|_| ToolError::BadInput("port must be 1..65535".into()))?;
    if port == 0 {
        return Err(ToolError::BadInput("port must be 1..65535".into()));
    }
    Ok(port)
}

fn capped_timeout(input: &Value) -> u64 {
    input
        .get("timeout_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(HOST_NET_REACH_DEFAULT_TIMEOUT_MS)
        .clamp(1, HOST_NET_REACH_MAX_TIMEOUT_MS)
}

fn probe(host: String, port: u16, timeout_ms: u64) -> Result<HostNetReach, ToolError> {
    let timeout = Duration::from_millis(timeout_ms);
    let started = Instant::now();
    let addrs: Vec<SocketAddr> = match (host.as_str(), port).to_socket_addrs() {
        Ok(addrs) => addrs.take(MAX_RESOLVED_ADDRS).collect(),
        Err(_) => {
            return Ok(HostNetReach {
                host,
                port,
                reachable: false,
                latency_ms: None,
                timeout_ms,
                error: Some("resolve_failed".to_string()),
            });
        }
    };
    if addrs.is_empty() {
        return Ok(HostNetReach {
            host,
            port,
            reachable: false,
            latency_ms: None,
            timeout_ms,
            error: Some("resolve_failed".to_string()),
        });
    }

    let mut last_error = "unreachable".to_string();
    for addr in addrs {
        let elapsed = started.elapsed();
        if elapsed >= timeout {
            last_error = "timeout".to_string();
            break;
        }
        match TcpStream::connect_timeout(&addr, timeout - elapsed) {
            Ok(_) => {
                return Ok(HostNetReach {
                    host,
                    port,
                    reachable: true,
                    latency_ms: Some(started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)),
                    timeout_ms,
                    error: None,
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                last_error = "timeout".to_string()
            }
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                last_error = "connection_refused".to_string()
            }
            Err(e) => last_error = e.kind().to_string(),
        }
    }

    Ok(HostNetReach {
        host,
        port,
        reachable: false,
        latency_ms: None,
        timeout_ms,
        error: Some(last_error),
    })
}
