//! `fleet-monitor` end to end (ui-federation + dashboard-widgets + native-tier scopes): the
//! reference SELF-CONTAINED extension — backend + frontend in ONE folder — proven through the REAL
//! native install path with a REAL supervised OS child (its own PID). Mocks nothing: real embedded
//! SurrealDB, real supervisor, the real `fleet-monitor` binary.
//!
//! The proof, end to end:
//!   1. install the native `fleet-monitor` → its child spawns and answers `fleet.summary`, tagged
//!      with the injected workspace identity (the scoped env reached the child — its own PID);
//!   2. `ext.list` surfaces BOTH UI halves the manifest declared — the `[ui]` PAGE (one nav slot) and
//!      the TWO `[[widget]]` palette tiles — even though this is a NATIVE extension (a page/widget is
//!      independent of the runtime tier; the native install path persists them like the wasm path);
//!   3. each UI scope is narrowed to the grant (a page/widget never claims an ungranted tool).
//!
//! This is the load-bearing fix's regression test: before this slice the native install path did NOT
//! persist `[ui]`/`[widget]`, so a native extension's page/widgets silently never surfaced.

use std::path::PathBuf;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_sidecar, ext_list, install_native, Node};
use lb_supervisor::OsLauncher;

const MANIFEST: &str = include_str!("../../../extensions/fleet-monitor/extension.toml");

/// The dir holding the built reference sidecar binary (cargo target). Panics with the build hint.
fn sidecar_dir() -> String {
    if let Ok(p) = std::env::var("FLEET_MONITOR_BIN") {
        return PathBuf::from(p)
            .parent()
            .unwrap()
            .to_string_lossy()
            .into_owned();
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug");
    if !dir.join("fleet-monitor").exists() {
        panic!(
            "missing fleet-monitor at {} — run: (cd rust && cargo build -p fleet-monitor)",
            dir.join("fleet-monitor").display()
        );
    }
    dir.to_string_lossy().into_owned()
}

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:ada".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn native_install_spawns_child_and_surfaces_page_plus_two_widgets() {
    let ws = "acme";
    let node = Node::boot().await.unwrap();
    let launcher = OsLauncher;
    let admin = principal(
        ws,
        &[
            "mcp:native.install:call",
            "mcp:native.call:call",
            "mcp:ext.list:call",
        ],
    );
    // Approve all three series read verbs the manifest requests, so every UI scope survives narrowing.
    let approved = vec![
        "mcp:series.find:call".to_string(),
        "mcp:series.latest:call".to_string(),
        "mcp:series.read:call".to_string(),
    ];

    // --- 1. install (spawn) the native child; it answers its own MCP tool, tagged with the ws ---
    let supervised = install_native(
        &node,
        &launcher,
        &admin,
        ws,
        MANIFEST,
        &sidecar_dir(),
        &approved,
        1,
    )
    .await
    .expect("native fleet-monitor installs + spawns");
    assert_eq!(supervised.version, "0.1.0");
    assert_eq!(supervised.tools, vec!["fleet.summary".to_string()]);

    let out = call_sidecar(
        &node,
        &launcher,
        &admin,
        ws,
        "fleet-monitor",
        "fleet.summary",
        r#""{}""#,
        1,
    )
    .await
    .expect("fleet.summary answers");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["ws"], ws, "the injected LB_EXT_WS reached the child");
    assert_eq!(v["tier"], "native");

    // --- 2. ext.list surfaces the PAGE + BOTH widget tiles for this NATIVE extension ---
    let rows = ext_list(&node, &admin, ws).await.expect("ext.list");
    let row = rows
        .iter()
        .find(|r| r.ext == "fleet-monitor")
        .expect("fleet-monitor row present");
    assert_eq!(row.tier, "native");

    let page = row.ui.as_ref().expect("the [ui] page surfaced");
    assert_eq!(page.entry, "assets/remoteEntry.js");
    assert_eq!(page.label, "Fleet Monitor");
    // --- 3. the page scope is narrowed to the grant (all three approved here) ---
    assert_eq!(
        page.scope,
        vec![
            "series.find".to_string(),
            "series.latest".to_string(),
            "series.read".to_string()
        ]
    );

    assert_eq!(row.widgets.len(), 2, "both [[widget]] tiles surfaced");
    assert_eq!(row.widgets[0].label, "Fleet Status");
    assert_eq!(row.widgets[0].scope, vec!["series.latest".to_string()]);
    assert_eq!(row.widgets[1].label, "Fleet Sparkline");
    assert_eq!(row.widgets[1].scope, vec!["series.read".to_string()]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn native_ui_scope_is_narrowed_to_the_grant() {
    // The manifest requests three series verbs, but the admin approves ONLY series.latest. The
    // persisted page + widget scopes must drop the un-approved verbs — a native page/widget is a
    // gated caller, exactly like a wasm one.
    let ws = "acme";
    let node = Node::boot().await.unwrap();
    let launcher = OsLauncher;
    let admin = principal(ws, &["mcp:native.install:call", "mcp:ext.list:call"]);
    let approved = vec!["mcp:series.latest:call".to_string()];

    install_native(
        &node,
        &launcher,
        &admin,
        ws,
        MANIFEST,
        &sidecar_dir(),
        &approved,
        1,
    )
    .await
    .expect("installs");

    let rows = ext_list(&node, &admin, ws).await.unwrap();
    let row = rows.iter().find(|r| r.ext == "fleet-monitor").unwrap();
    // Page: only series.latest survives.
    assert_eq!(
        row.ui.as_ref().unwrap().scope,
        vec!["series.latest".to_string()]
    );
    // Widget 0 (series.latest) keeps its scope; widget 1 (series.read, ungranted) is emptied.
    assert_eq!(row.widgets[0].scope, vec!["series.latest".to_string()]);
    assert!(
        row.widgets[1].scope.is_empty(),
        "the series.read widget loses its ungranted scope"
    );
}
