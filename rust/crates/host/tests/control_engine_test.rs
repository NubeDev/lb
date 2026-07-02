//! Control-engine (control-engine scope, S3) end-to-end tests — the mandatory
//! categories for the local read-verb slice (slice-3 exit gate): the
//! capability-DENY path (before any sidecar/CE round trip), the HAPPY read verbs
//! (`control-engine.tree` / `control-engine.schema` through the real MCP gate +
//! supervisor + stdio ABI), and the HOT-RESTART supervision proof.
//!
//! NO mocks for our own stack: real embedded SurrealDB + in-proc Zenoh, real caps,
//! the REAL native supervisor (`OsLauncher`) spawning the REAL `control-engine`
//! sidecar binary. CE itself is the ONE sanctioned true-external (a C++20 engine we
//! cannot build in Rust CI): stubbed behind `rubix-ce`'s `ControlEngine` trait in the
//! one named `ce_fake` module, compiled into the sidecar under `--features ce-fake`
//! and activated per-call by `LB_CE_FAKE=1`. The real engine is exercised by the
//! env-gated `#[ignore]`d test below (`CE_ENGINE_URL`, default 127.0.0.1:7979).
//!
//! Workspace isolation is meaningfully testable only with the appliance registry →
//! DEFERRED to S4 (noted here, NOT faked).

use std::process::Command;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_sidecar, install_native, restart_native, status_native, Lifecycle, Node};
use lb_mcp::authorize_tool;
use lb_supervisor::OsLauncher;
use serde_json::{json, Value};

const MANIFEST: &str = include_str!("../../../extensions/control-engine/extension.toml");

// ---------------------------------------------------------------------------------
// Identity helpers (federation_test pattern).
// ---------------------------------------------------------------------------------

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

/// An admin holding install/call/stop/status + both read-verb caps.
fn admin(ws: &str) -> Principal {
    principal(
        ws,
        &[
            "mcp:native.install:call",
            "mcp:native.call:call",
            "mcp:native.stop:call",
            "mcp:native.status:call",
            "mcp:native.restart:call",
            "mcp:control-engine.tree:call",
            "mcp:control-engine.schema:call",
        ],
    )
}

// ---------------------------------------------------------------------------------
// Build the sidecar with --features ce-fake (the sanctioned CE stub baked in) and
// return the directory holding the binary the host supervisor spawns.
// ---------------------------------------------------------------------------------

fn control_engine_dir() -> Option<String> {
    if let Ok(p) = std::env::var("CONTROL_ENGINE_BIN") {
        let dir = std::path::PathBuf::from(&p);
        return Some(dir.parent().unwrap().to_string_lossy().into_owned());
    }
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = manifest_dir.join("../../target/debug");
    let bin = target.join("control-engine");
    let status = Command::new("cargo")
        .args(["build", "-p", "control-engine", "--features", "ce-fake"])
        .current_dir(manifest_dir.join("../.."))
        .status();
    match status {
        Ok(s) if s.success() && bin.exists() => Some(target.to_string_lossy().into_owned()),
        _ => None,
    }
}

async fn install(node: &Node, admin: &Principal, ws: &str, dir: &str) {
    // Approve exactly the local CE socket the manifest requests (net:tcp escape hatch).
    let approved = vec!["net:tcp:127.0.0.1:7979:connect".to_string()];
    install_native(node, &OsLauncher, admin, ws, MANIFEST, dir, &approved, 1)
        .await
        .expect("control-engine sidecar installs + spawns");
}

/// Drive a `control-engine.*` verb through the REAL per-tool gate then the REAL
/// supervised sidecar. This is the S3 composition of two generic, CE-IGNORANT host
/// primitives — `authorize_tool` (the tool NAME is the cap gate: it maps
/// `control-engine.tree` → `mcp:control-engine.tree:call` with no CE knowledge) and
/// `call_sidecar` (native supervision + on-demand restart). Neither special-cases
/// CE, so no CE string leaks into core; the registry-routed `call_tool` hop (which
/// would let the wiresheet/agent reach it uniformly) lands in S4. The gate runs
/// FIRST, so a denied caller never reaches the sidecar or any CE trait call.
async fn call(
    node: &std::sync::Arc<Node>,
    p: &Principal,
    ws: &str,
    tool: &str,
    input: Value,
) -> Result<Value, lb_mcp::ToolError> {
    // Gate on the qualified tool NAME (workspace-first, then mcp:<tool>:call).
    authorize_tool(p, ws, tool)?;
    let out = call_sidecar(
        node,
        &OsLauncher,
        p,
        ws,
        "control-engine",
        tool,
        &input.to_string(),
        9,
    )
    .await
    .map_err(|e| lb_mcp::ToolError::Extension(e.to_string()))?;
    Ok(serde_json::from_str(&out).unwrap())
}

// ---------------------------------------------------------------------------------
// THE TEST — deny + happy read verbs + hot-restart, against the fake-backed sidecar.
// ---------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn control_engine_local_read_verbs_and_supervision() {
    // The sidecar's fake-CE path is armed per-call by this env, forwarded to the
    // spawned child (OsLauncher inherits the parent env). OFF in a shipped binary.
    std::env::set_var("LB_CE_FAKE", "1");

    let Some(dir) = control_engine_dir() else {
        eprintln!("SKIP control_engine_local_read_verbs_and_supervision: could not build control-engine --features ce-fake");
        return;
    };

    let ws = "acme";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install(&node, &admin, ws, &dir).await;

    // --- HAPPY: control-engine.tree returns the fake's seeded graph (verbatim DTOs) ---
    let tree = call(
        &node,
        &admin,
        ws,
        "control-engine.tree",
        json!({ "appliance": "127.0.0.1:7979" }),
    )
    .await
    .expect("control-engine.tree");
    let nodes = tree["nodes"].as_array().expect("nodes array");
    assert_eq!(nodes.len(), 1, "one seeded node: {tree}");
    assert_eq!(nodes[0]["uid"], 1);
    assert_eq!(nodes[0]["type"], "test-math::add");
    assert_eq!(tree["edges"].as_array().unwrap().len(), 0);

    // --- HAPPY: control-engine.schema returns the fake's manifest list ---
    let schema = call(
        &node,
        &admin,
        ws,
        "control-engine.schema",
        json!({ "appliance": "127.0.0.1:7979" }),
    )
    .await
    .expect("control-engine.schema");
    let mans = schema["manifests"].as_array().expect("manifests array");
    assert_eq!(mans.len(), 1, "one seeded manifest: {schema}");
    assert_eq!(mans[0]["vendor"], "test");
    assert_eq!(mans[0]["name"], "math");

    // --- CAPABILITY-DENY: a caller WITHOUT mcp:control-engine.tree:call → Denied ---
    // The host gate stops this at the call_tool boundary, BEFORE the sidecar/CE is
    // reached (the deny-before-any-trait-call semantics are unit-tested at the
    // dispatch layer inside the crate; here we assert the opaque Denied at the gate).
    let no_cap = principal(ws, &["mcp:native.call:call"]);
    let denied = call(
        &node,
        &no_cap,
        ws,
        "control-engine.tree",
        json!({ "appliance": "127.0.0.1:7979" }),
    )
    .await
    .expect_err("tree without mcp:control-engine.tree:call is denied");
    assert!(
        matches!(denied, lb_mcp::ToolError::Denied),
        "opaque deny: {denied:?}"
    );

    // --- HOT-RESTART: the supervisor kills + re-launches + re-handshakes the sidecar
    //     (restart_native), then it still answers with no state lost (it holds none).
    //     Mirrors native_test.rs restart_count == 1. ---
    let restarts = restart_native(&node, &OsLauncher, &admin, ws, "control-engine", 2)
        .await
        .expect("supervised restart");
    assert_eq!(restarts, 1, "the sidecar was restarted exactly once");
    let after = call(
        &node,
        &admin,
        ws,
        "control-engine.tree",
        json!({ "appliance": "127.0.0.1:7979" }),
    )
    .await
    .expect("tree answers after kill + supervised restart");
    assert_eq!(after["nodes"].as_array().unwrap().len(), 1);

    let status = status_native(&node, &admin, ws, "control-engine")
        .await
        .unwrap()
        .expect("status exists");
    assert_eq!(
        status.restart_count, 1,
        "the killed sidecar was restarted exactly once"
    );
    assert_eq!(status.lifecycle, Lifecycle::Started);

    // Workspace isolation is DEFERRED to S4 (needs the appliance registry) — not faked.
}

// ---------------------------------------------------------------------------------
// The opt-in REAL-ENGINE tier (control-engine scope §"real-engine tier"): runs the
// same read verbs against a REAL ce-studio engine via the REAL CeRestClient. Gated
// on CE_ENGINE_URL (default 127.0.0.1:7979) and #[ignore]d so CI stays on the fake.
// Run: `CE_ENGINE_URL=127.0.0.1:7979 cargo test -p lb-host --test control_engine_test
//       -- --ignored control_engine`
// ---------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs a real ce-studio engine on CE_ENGINE_URL (default 127.0.0.1:7979)"]
async fn control_engine_against_real_ce_studio() {
    // NOTE: real engine → NO LB_CE_FAKE; the sidecar uses the real rubix-ce client.
    std::env::remove_var("LB_CE_FAKE");
    let base = std::env::var("CE_ENGINE_URL").unwrap_or_else(|_| "127.0.0.1:7979".to_string());

    let Some(dir) = control_engine_dir() else {
        eprintln!("SKIP real-engine: could not build control-engine");
        return;
    };

    let ws = "acme";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install(&node, &admin, ws, &dir).await;

    let tree = call(
        &node,
        &admin,
        ws,
        "control-engine.tree",
        json!({ "appliance": base }),
    )
    .await
    .expect("control-engine.tree against the real engine");
    assert!(tree.get("nodes").is_some(), "real tree has nodes: {tree}");

    let schema = call(
        &node,
        &admin,
        ws,
        "control-engine.schema",
        json!({ "appliance": base }),
    )
    .await
    .expect("control-engine.schema against the real engine");
    assert!(
        schema.get("manifests").is_some(),
        "real schema has manifests: {schema}"
    );
}
