//! The **embed API** integration test (node-roles / embed scope): boot a real node through the
//! supported `lb_node::boot_full(BootConfig)` seam — the same code path the `node` binary runs — with
//! the gateway OFF and reactors OFF, then prove the two mandatory guarantees still hold through the
//! embedded boot: a capability-DENY (a caller without the cap is refused on a host verb) and
//! WORKSPACE-ISOLATION (ws-B cannot read ws-A rows). Real infra: a `mem://` store, the real host
//! verbs, real signed principals — no mocks (CLAUDE §9 / testing §0).
//!
//! `hello_demo` is OFF (an embedded boot doesn't want the demo extension), `seed_user` is `None` (the
//! test provisions its own principals), so the boot is the minimal store+auth+MCP subset an embedder
//! asks for. That subset boot itself is the parity assertion: it completes and the node serves verbs.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_ingest_tool, drain_workspace};
use lb_mcp::ToolError;
use lb_node::{boot_full, BootConfig};
use serde_json::json;

/// A real signed principal for `ws` carrying `caps`. Same helper the host isolation tests use.
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
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// A headless embed config: `mem://` store, gateway OFF, reactors OFF, hello demo OFF, no dev seed —
/// the store+auth+MCP-only subset an embedder wants. Constructs `BootConfig` directly (no env).
fn embed_config() -> BootConfig {
    // `BootConfig` is `#[non_exhaustive]`, so a downstream crate mutates fields on `default()` rather
    // than a struct literal — the additive-fields embed contract (a new field never breaks this call).
    let mut cfg = BootConfig::default();
    cfg.seed_user = None;
    cfg.reactors = false;
    cfg.hello_demo = false;
    cfg
}

/// MANDATORY capability-deny: through an embedded boot, a caller WITHOUT the write cap is `Denied` on a
/// host verb (`ingest.write`), and a caller WITH it succeeds — the wall is intact under `boot_full`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn embedded_node_denies_a_caller_without_the_cap() {
    let running = boot_full(embed_config()).await.expect("embedded boot");
    let store = &running.node.store;

    // No write cap → Denied at the capability gate (workspace matches, so it's the cap, not gate 1).
    let no_cap = principal("client:reader", "acme", &["mcp:series.read:call"]);
    let denied = call_ingest_tool(
        store,
        &no_cap,
        "acme",
        "ingest.write",
        &json!({ "samples": [{ "series": "m", "producer": "x", "ts": 1, "seq": 1, "payload": 1, "qos": "must-deliver" }] }),
    )
    .await
    .unwrap_err();
    assert!(matches!(denied, ToolError::Denied), "no cap ⇒ Denied");

    // With the cap → the same call succeeds (the verb is really wired through the embedded node).
    let writer = principal("client:writer", "acme", &["mcp:ingest.write:call"]);
    call_ingest_tool(
        store,
        &writer,
        "acme",
        "ingest.write",
        &json!({ "samples": [{ "series": "m", "producer": "x", "ts": 1, "seq": 1, "payload": 1, "qos": "must-deliver" }] }),
    )
    .await
    .expect("with the cap the write is authorized");
}

/// MANDATORY workspace-isolation: through an embedded boot, ws-B cannot read ws-A rows. Gate 1
/// (workspace) fires before the capability is consulted — a ws-B token asking for ws-A is `Denied`,
/// and a ws-B token reading its OWN namespace sees nothing of A's.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn embedded_node_isolates_workspaces() {
    let running = boot_full(embed_config()).await.expect("embedded boot");
    let store = &running.node.store;

    const ALL: &[&str] = &[
        "mcp:ingest.write:call",
        "mcp:series.read:call",
        "mcp:series.latest:call",
    ];

    // ws-A writes a secret series and it commits in A.
    let a = principal("client:a", "ws-a", ALL);
    call_ingest_tool(
        store,
        &a,
        "ws-a",
        "ingest.write",
        &json!({ "samples": [{ "series": "secret", "producer": "x", "ts": 1, "seq": 1, "payload": 42, "qos": "must-deliver" }] }),
    )
    .await
    .unwrap();
    drain_workspace(store, "ws-a").await.unwrap();

    // A ws-B token reading B's own "secret" sees nothing of A's.
    let b = principal("client:a", "ws-b", ALL);
    let read = call_ingest_tool(
        store,
        &b,
        "ws-b",
        "series.read",
        &json!({ "series": "secret" }),
    )
    .await
    .unwrap();
    assert!(
        read["samples"].as_array().unwrap().is_empty(),
        "ws-B must not see ws-A samples"
    );

    // And a ws-B token asking for ws-A's namespace is refused at gate 1 (workspace), opaque Denied.
    let cross = call_ingest_tool(
        store,
        &b,
        "ws-a",
        "series.read",
        &json!({ "series": "secret" }),
    )
    .await
    .unwrap_err();
    assert!(matches!(cross, ToolError::Denied), "cross-ws read ⇒ Denied");
}

/// `BootConfig::from_env` with a clean env reproduces the binary's defaults (workspace `acme`, dev seed
/// on, gateway off, reactors on, hello demo ON) — the parity guard that the env seam matches today.
#[test]
fn from_env_defaults_match_the_binary() {
    // No LB_* set in the test process → the documented defaults.
    let cfg = BootConfig::from_env();
    assert_eq!(cfg.workspace, "acme");
    assert_eq!(cfg.seed_user.as_deref(), Some("user:ada"));
    assert!(cfg.reactors, "reactors default on for the binary");
    assert!(cfg.hello_demo, "the binary loads the hello demo");
    assert!(matches!(cfg.gateway, lb_node::GatewayMode::Off));
    assert!(cfg.store_path.is_none(), "no LB_STORE_PATH ⇒ mem store");
}
