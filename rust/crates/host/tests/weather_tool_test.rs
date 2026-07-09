//! `weather.current`, headless over the real MCP bridge (testing §0/§2.1, rule 9).
//!
//! Open-Meteo is the one sanctioned external fake-boundary: a real local HTTP server (a bare
//! `TcpListener`, no mock library) serves a canned Open-Meteo-shaped response behind
//! `OPEN_METEO_BASE_ENV`, and `weather_current` runs a real `reqwest` GET against it — no
//! hand-written re-implementation of the tool's own behavior.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{Node, OPEN_METEO_BASE_ENV};
use lb_mcp::ToolError;
use serde_json::{json, Value};

const WEATHER_CURRENT: &str = "mcp:weather.current:call";

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

async fn call(
    node: &std::sync::Arc<Node>,
    principal: &Principal,
    ws: &str,
    tool: &str,
    input: Value,
) -> Result<Value, ToolError> {
    let raw = lb_host::call_tool(node, principal, ws, tool, &input.to_string()).await?;
    serde_json::from_str(&raw).map_err(|e| ToolError::Extension(e.to_string()))
}

/// Serve one canned Open-Meteo `current=` response on an ephemeral loopback port. A real socket, a
/// real HTTP/1.1 response — not a mocked client. Detached; the test process reaps it on exit.
fn serve_current(body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub listener");
    let addr = listener.local_addr().expect("local addr");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { break };
            handle_one(stream, body);
        }
    });
    format!("http://{addr}")
}

fn handle_one(mut stream: TcpStream, body: &str) {
    let mut buf = [0u8; 4096];
    let _ = stream.read(&mut buf);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

// `time` is a UTC epoch (SECONDS) — the node asks Open-Meteo for `timeformat=unixtime`. 1783598400 =
// 2026-07-09T12:00:00Z. The UI renders it in the viewer's browser timezone.
const CANNED_BODY: &str = r#"{"current":{"time":1783598400,"temperature_2m":21.4,"wind_speed_10m":11.2,"weather_code":3}}"#;

/// A guard that restores (or clears) the shared process-wide env var on drop, so tests running in
/// the same binary (`cargo test` = one process) don't leak a stub base into a sibling test.
struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var(key).ok();
        // SAFETY: tests in this file are serialized on this var via `#[serial]`-free but exclusive
        // use — each test sets, calls, then restores before the next test's `Node::boot()` runs.
        unsafe { std::env::set_var(key, value) };
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(v) => unsafe { std::env::set_var(self.key, v) },
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn current_reads_a_real_local_stub_shaped_like_open_meteo() {
    let base = serve_current(CANNED_BODY);
    let _guard = EnvGuard::set(OPEN_METEO_BASE_ENV, &base);

    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let ws = "weather-happy";
    let p = principal("user:ada", ws, &[WEATHER_CURRENT]);

    let out = call(
        &node,
        &p,
        ws,
        "weather.current",
        json!({ "lat": -27.47, "lon": 153.02 }),
    )
    .await
    .unwrap();

    assert_eq!(out["temp_c"], 21.4);
    assert_eq!(out["wind_kph"], 11.2);
    assert_eq!(out["code"], 3);
    assert_eq!(out["observed_ts"], 1783598400); // UTC epoch seconds (2026-07-09T12:00:00Z)
    assert_eq!(out["location"], "-27.47,153.02");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn current_without_its_cap_is_denied_before_any_fetch() {
    // No stub base set: if the deny gate did not run first, the call would try a real network
    // fetch and fail with an ambiguous error instead of `ToolError::Denied`.
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let ws = "weather-deny";
    let p = principal("user:nobody", ws, &[]);

    let err = call(
        &node,
        &p,
        ws,
        "weather.current",
        json!({ "lat": 0.0, "lon": 0.0 }),
    )
    .await
    .unwrap_err();
    assert_eq!(err, ToolError::Denied);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn current_missing_args_is_bad_input_not_denied() {
    let base = serve_current(CANNED_BODY);
    let _guard = EnvGuard::set(OPEN_METEO_BASE_ENV, &base);

    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let ws = "weather-badinput";
    let p = principal("user:ada", ws, &[WEATHER_CURRENT]);

    let err = call(&node, &p, ws, "weather.current", json!({ "lat": 1.0 }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::BadInput(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn other_workspace_token_is_denied() {
    let base = serve_current(CANNED_BODY);
    let _guard = EnvGuard::set(OPEN_METEO_BASE_ENV, &base);

    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ben", "ws-b", &[WEATHER_CURRENT]);
    let err = call(
        &node,
        &p,
        "ws-a",
        "weather.current",
        json!({ "lat": 1.0, "lon": 2.0 }),
    )
    .await
    .unwrap_err();
    assert_eq!(err, ToolError::Denied);
}
