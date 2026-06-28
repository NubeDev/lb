//! Built-in `host.*` tools, headless over the real MCP bridge.
//!
//! Real infra only: `Node::boot()` (mem store + real caps), real OS clock/interfaces, a real temp
//! directory for filesystem metadata, and a real loopback `TcpListener` for reachability.

use std::collections::BTreeSet;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::Node;
use lb_mcp::ToolError;
use serde_json::{json, Value};

const NET_INFO: &str = "mcp:host.net.info:call";
const NET_REACH: &str = "mcp:host.net.reach:call";
const TIME_NOW: &str = "mcp:host.time.now:call";
const TIME_ZONES: &str = "mcp:host.time.zones:call";
const FS_STAT: &str = "mcp:host.fs.stat:call";
const FS_LIST: &str = "mcp:host.fs.list:call";

static TEMP_ID: AtomicUsize = AtomicUsize::new(0);

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

async fn call(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    tool: &str,
    input: Value,
) -> Result<Value, ToolError> {
    let raw = lb_host::call_tool(node, principal, ws, tool, &input.to_string()).await?;
    serde_json::from_str(&raw).map_err(|e| ToolError::Extension(e.to_string()))
}

fn temp_tree(name: &str) -> PathBuf {
    let id = TEMP_ID.fetch_add(1, Ordering::SeqCst);
    let root =
        std::env::temp_dir().join(format!("lb-host-tools-{name}-{}-{id}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("child-dir")).expect("temp dir");
    std::fs::write(root.join("payload.txt"), b"DO_NOT_LEAK_HOST_TOOLS_SECRET").expect("temp file");
    root
}

fn keys(value: &Value) -> BTreeSet<String> {
    value
        .as_object()
        .expect("json object")
        .keys()
        .cloned()
        .collect()
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn net_info_without_its_cap_is_denied() {
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:nobody", "host-deny-net-info", &[]);
    let err = call(&node, &p, "host-deny-net-info", "host.net.info", json!({}))
        .await
        .unwrap_err();
    assert_eq!(err, ToolError::Denied);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn net_reach_without_its_cap_is_denied() {
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:nobody", "host-deny-net-reach", &[]);
    let err = call(
        &node,
        &p,
        "host-deny-net-reach",
        "host.net.reach",
        json!({ "host": "127.0.0.1", "port": 1 }),
    )
    .await
    .unwrap_err();
    assert_eq!(err, ToolError::Denied);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn time_now_without_its_cap_is_denied() {
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:nobody", "host-deny-time-now", &[]);
    let err = call(&node, &p, "host-deny-time-now", "host.time.now", json!({}))
        .await
        .unwrap_err();
    assert_eq!(err, ToolError::Denied);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn time_zones_without_its_cap_is_denied() {
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:nobody", "host-deny-time-zones", &[]);
    let err = call(
        &node,
        &p,
        "host-deny-time-zones",
        "host.time.zones",
        json!({}),
    )
    .await
    .unwrap_err();
    assert_eq!(err, ToolError::Denied);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn fs_stat_without_its_cap_is_denied() {
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:nobody", "host-deny-fs-stat", &[]);
    let err = call(
        &node,
        &p,
        "host-deny-fs-stat",
        "host.fs.stat",
        json!({ "path": "." }),
    )
    .await
    .unwrap_err();
    assert_eq!(err, ToolError::Denied);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn fs_list_without_its_cap_is_denied() {
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:nobody", "host-deny-fs-list", &[]);
    let err = call(
        &node,
        &p,
        "host-deny-fs-list",
        "host.fs.list",
        json!({ "path": "." }),
    )
    .await
    .unwrap_err();
    assert_eq!(err, ToolError::Denied);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn other_workspace_token_is_denied_before_node_global_fact_is_read() {
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ben", "ws-b", &[TIME_NOW]);
    let err = call(&node, &p, "ws-a", "host.time.now", json!({}))
        .await
        .unwrap_err();
    assert_eq!(err, ToolError::Denied);

    let no_ws = principal("user:no-ws", "", &[TIME_NOW]);
    let err = call(&node, &no_ws, "ws-a", "host.time.now", json!({}))
        .await
        .unwrap_err();
    assert_eq!(err, ToolError::Denied);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn every_verb_returns_the_same_shape_on_this_os() {
    let node = Arc::new(Node::boot().await.unwrap());
    let ws = "host-shape";
    let p = principal(
        "user:shape",
        ws,
        &[NET_INFO, NET_REACH, TIME_NOW, TIME_ZONES, FS_STAT, FS_LIST],
    );
    let root = temp_tree("shape");
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let port = listener.local_addr().unwrap().port();

    let net_info = call(&node, &p, ws, "host.net.info", json!({}))
        .await
        .unwrap();
    assert_eq!(keys(&net_info), set(&["hostname", "interfaces"]));
    for iface in net_info["interfaces"].as_array().unwrap() {
        assert_eq!(keys(iface), set(&["name", "addresses"]));
        for addr in iface["addresses"].as_array().unwrap() {
            assert_eq!(keys(addr), set(&["ip", "family", "scope"]));
        }
    }

    let reach = call(
        &node,
        &p,
        ws,
        "host.net.reach",
        json!({ "host": "127.0.0.1", "port": port, "timeout_ms": 500 }),
    )
    .await
    .unwrap();
    assert_eq!(
        keys(&reach),
        set(&[
            "host",
            "port",
            "reachable",
            "latency_ms",
            "timeout_ms",
            "error"
        ])
    );

    let now = call(&node, &p, ws, "host.time.now", json!({}))
        .await
        .unwrap();
    assert_eq!(keys(&now), set(&["utc", "local", "zone", "offset_seconds"]));

    let zones = call(&node, &p, ws, "host.time.zones", json!({}))
        .await
        .unwrap();
    assert_eq!(keys(&zones), set(&["zones", "count"]));
    assert!(zones["zones"]
        .as_array()
        .unwrap()
        .iter()
        .any(|z| z == "UTC"));

    let stat = call(
        &node,
        &p,
        ws,
        "host.fs.stat",
        json!({ "path": root.join("payload.txt") }),
    )
    .await
    .unwrap();
    assert_eq!(
        keys(&stat),
        set(&["path", "os", "exists", "kind", "size", "mtime", "readable", "writable"])
    );

    let list = call(&node, &p, ws, "host.fs.list", json!({ "path": root }))
        .await
        .unwrap();
    assert_eq!(keys(&list), set(&["path", "os", "entries", "truncated"]));
    for entry in list["entries"].as_array().unwrap() {
        assert_eq!(keys(entry), set(&["name", "kind", "size"]));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn reach_uses_a_bounded_tcp_probe_and_rejects_port_ranges() {
    let node = Arc::new(Node::boot().await.unwrap());
    let ws = "host-reach";
    let p = principal("user:reach", ws, &[NET_REACH]);

    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let open_port = listener.local_addr().unwrap().port();
    let open = call(
        &node,
        &p,
        ws,
        "host.net.reach",
        json!({ "host": "127.0.0.1", "port": open_port, "timeout_ms": 500 }),
    )
    .await
    .unwrap();
    assert_eq!(open["reachable"], true);

    let closed = TcpListener::bind("127.0.0.1:0").expect("closed listener");
    let closed_port = closed.local_addr().unwrap().port();
    drop(closed);
    let started = Instant::now();
    let out = call(
        &node,
        &p,
        ws,
        "host.net.reach",
        json!({ "host": "127.0.0.1", "port": closed_port, "timeout_ms": 250 }),
    )
    .await
    .unwrap();
    assert_eq!(out["reachable"], false);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "closed local probe must not hang"
    );

    let err = call(
        &node,
        &p,
        ws,
        "host.net.reach",
        json!({ "host": "127.0.0.1", "port": "1-3" }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::BadInput(_)));

    let err = call(
        &node,
        &p,
        ws,
        "host.net.reach",
        json!({ "host": "127.0.0.1", "port": 0 }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::BadInput(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn dto_allow_lists_leak_no_file_contents_or_extra_network_fields() {
    let node = Arc::new(Node::boot().await.unwrap());
    let ws = "host-leak";
    let p = principal("user:leak", ws, &[NET_INFO, FS_STAT, FS_LIST]);
    let root = temp_tree("leak");

    let stat = call(
        &node,
        &p,
        ws,
        "host.fs.stat",
        json!({ "path": root.join("payload.txt") }),
    )
    .await
    .unwrap();
    let list = call(&node, &p, ws, "host.fs.list", json!({ "path": root }))
        .await
        .unwrap();
    let stat_raw = stat.to_string();
    let list_raw = list.to_string();
    assert!(!stat_raw.contains("DO_NOT_LEAK_HOST_TOOLS_SECRET"));
    assert!(!list_raw.contains("DO_NOT_LEAK_HOST_TOOLS_SECRET"));
    assert_eq!(
        keys(&stat),
        set(&["path", "os", "exists", "kind", "size", "mtime", "readable", "writable"])
    );
    assert_eq!(keys(&list), set(&["path", "os", "entries", "truncated"]));

    let net = call(&node, &p, ws, "host.net.info", json!({}))
        .await
        .unwrap();
    assert_eq!(keys(&net), set(&["hostname", "interfaces"]));
    assert!(!net.to_string().contains("routes"));
    assert!(!net.to_string().contains("env"));
}
