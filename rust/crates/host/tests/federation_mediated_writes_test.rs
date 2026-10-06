//! `federation.write` / `federation.migrate` / `federation.delete` are HOST-MEDIATED, like
//! `federation.query`: a caller needs the verb's own cap and NOT the supervisor control-plane cap
//! `mcp:native.call:call`. Before, an extension that only wanted to create and write its own tables
//! through federation had to be granted "drive any native child".
//!
//! Real node, real supervisor, real federation sidecar on a sqlite file — no mocks.

use std::process::Command;
use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, install_native, Node};
use lb_supervisor::OsLauncher;
use serde_json::{json, Value};

const MANIFEST: &str = include_str!("../../federation/extension.toml");
const WS: &str = "nube";

fn principal(sub: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: WS.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

/// Sets up the node: installs the sidecar and registers the source (control-plane caps needed).
fn installer() -> Principal {
    principal(
        "user:admin",
        &[
            "mcp:native.install:call",
            "mcp:native.call:call",
            "mcp:datasource.add:call",
            "secret:federation/*:write",
            "secret:federation/*:get",
        ],
    )
}

/// What an extension's token holds for its own tables: the data verbs, and NO `native.call`.
fn extension() -> Principal {
    principal(
        "ext:waste-management",
        &[
            "mcp:federation.migrate:call",
            "mcp:federation.write:call",
            "mcp:federation.delete:call",
            "mcp:federation.query:call",
        ],
    )
}

fn federation_dir() -> String {
    if let Ok(p) = std::env::var("FEDERATION_BIN") {
        let dir = std::path::PathBuf::from(&p);
        return dir.parent().unwrap().to_string_lossy().into_owned();
    }
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = manifest_dir.join("../../target/debug");
    let status = Command::new("cargo")
        .args(["build", "-p", "federation"])
        .current_dir(manifest_dir.join("../.."))
        .status()
        .expect("cargo build -p federation runs");
    assert!(status.success() && target.join("federation").exists());
    target.to_string_lossy().into_owned()
}

async fn call(
    node: &Arc<Node>,
    p: &Principal,
    tool: &str,
    input: Value,
) -> Result<Value, lb_mcp::ToolError> {
    let out = call_tool(node, p, WS, tool, &input.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap())
}

async fn setup() -> Arc<Node> {
    let dir = federation_dir();
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = installer();
    let approved = vec![
        "net:tls:127.0.0.1:0:connect".to_string(),
        "secret:federation/*:get".to_string(),
    ];
    install_native(&node, &OsLauncher, &admin, WS, MANIFEST, &dir, &approved, 1)
        .await
        .expect("federation sidecar installs");
    let db = std::env::temp_dir().join(format!("lb-mediated-writes-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&db);
    rusqlite::Connection::open(&db).unwrap();
    call(
        &node,
        &admin,
        "datasource.add",
        json!({"name": "estate", "kind": "sqlite", "endpoint": "127.0.0.1:0", "dsn": db.to_string_lossy(), "ts": 1}),
    )
    .await
    .expect("datasource.add");
    node
}

fn schema() -> Value {
    json!({"tables": [{"name": "entry", "pk": ["id"], "columns": [
        {"name": "id", "type": "text"}, {"name": "kg", "type": "real", "nullable": true}
    ]}]})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_caller_without_native_call_migrates_writes_and_deletes() {
    let node = setup().await;
    let ext = extension();

    let m = call(
        &node,
        &ext,
        "federation.migrate",
        json!({"source": "estate", "schema": schema(), "dry_run": false}),
    )
    .await
    .expect("migrate with only mcp:federation.migrate:call");
    assert_eq!(m["applied"], json!(true), "{m}");

    call(
        &node,
        &ext,
        "federation.write",
        json!({"source": "estate", "table": "entry", "columns": ["id", "kg"], "rows": [["a", 1.5]], "key": ["id"]}),
    )
    .await
    .expect("write with only mcp:federation.write:call");

    call(
        &node,
        &ext,
        "federation.delete",
        json!({"source": "estate", "table": "entry", "key": ["id"], "rows": [["a"]]}),
    )
    .await
    .expect("delete with only mcp:federation.delete:call");
}

/// The verb gate is NOT weakened: without the verb's own cap, each is still refused.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_verb_still_needs_its_own_cap() {
    let node = setup().await;
    let reader = principal("user:viewer", &["mcp:federation.query:call"]);
    for (tool, input) in [
        (
            "federation.migrate",
            json!({"source": "estate", "schema": schema(), "dry_run": false}),
        ),
        (
            "federation.write",
            json!({"source": "estate", "table": "entry", "columns": ["id"], "rows": [["b"]]}),
        ),
        (
            "federation.delete",
            json!({"source": "estate", "table": "entry", "key": ["id"], "rows": [["b"]]}),
        ),
    ] {
        let err = call(&node, &reader, tool, input).await.unwrap_err();
        assert!(matches!(err, lb_mcp::ToolError::Denied), "{tool}: {err:?}");
    }
}
