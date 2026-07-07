//! Regression (agent-personas / tools-catalog): a `builtin.widget-builder` run must see the
//! data-analyst read surface it `extends` — `datasource.list`, `store.query`, `series.read`,
//! `viz.query` — in its narrowed menu.
//!
//! The original symptom (debugging/agent/persona-menu-missing-tools-catalog-descriptor-only.md):
//! `tools.catalog` served ONLY the ~11 verbs with palette descriptors, so `reachable_tools` could
//! never advertise the rest of the host-native inventory regardless of caps or persona, and the
//! in-house run under widget-builder was left with a 3-tool menu. Locks BOTH layers:
//!   1. `reachable_tools` (catalog ∩ caps) contains the full granted host-native set;
//!   2. the persona `extends` union + `narrow_tools` keeps the parent's surface in the menu.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    narrow_tools, reachable_tools, resolve_effective, resolve_persona, seed_personas, AllowedTool,
    Node,
};

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
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// The verbs the live symptom was missing — data-analyst surface a widget-builder run composes.
const WANT: &[&str] = &[
    "datasource.list",
    "datasource.test",
    "store.query",
    "store.schema",
    "series.read",
    "viz.query",
    "federation.query",
];

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reachable_tools_serves_full_host_inventory_and_persona_keeps_it() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_personas(&node.store).await.expect("seed");

    // A member holding the catalog gate + the data-analyst read caps (as the gateway dev login does).
    let caps: Vec<String> = std::iter::once("mcp:tools.catalog:call".to_string())
        .chain(WANT.iter().map(|t| format!("mcp:{t}:call")))
        .chain(["mcp:dashboard.pin:call".to_string()])
        .collect();
    let caps_ref: Vec<&str> = caps.iter().map(|s| s.as_str()).collect();
    let caller = principal("user:ada", "acme", &caps_ref);

    // Layer 1: the catalog-derived menu carries every granted host-native verb, not only the
    // descriptor-registered palette subset.
    let menu = reachable_tools(&node, &caller, "acme").await;
    let names: Vec<&str> = menu.iter().map(|t| t.name.as_str()).collect();
    for want in WANT {
        assert!(
            names.contains(want),
            "reachable_tools missing {want}: {names:?}"
        );
    }
    // An ungranted verb stays absent — the wall still shapes the menu.
    assert!(!names.contains(&"flows.create"));

    // Layer 2: the widget-builder persona (extends builtin.data-analyst) keeps the parent surface.
    let p = resolve_persona(&node, &caller, "acme", Some("builtin.widget-builder"))
        .await
        .expect("resolves")
        .expect("some persona");
    let ep = resolve_effective(&node, &caller, "acme", &p)
        .await
        .expect("effective");
    let narrowed = narrow_tools(&menu, &ep.granted_tools);
    let narrowed_names: Vec<&str> = narrowed.iter().map(|t| t.name.as_str()).collect();
    for want in WANT {
        assert!(
            narrowed_names.contains(want),
            "persona narrowed menu missing {want}: {narrowed_names:?}"
        );
    }
    assert!(narrowed_names.contains(&"dashboard.pin"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn widget_builder_unions_data_analyst_surface() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_personas(&node.store).await.expect("seed");
    let caller = principal("user:ada", "acme", &[]);

    let p = resolve_persona(&node, &caller, "acme", Some("builtin.widget-builder"))
        .await
        .expect("resolves")
        .expect("some persona");
    let ep = resolve_effective(&node, &caller, "acme", &p)
        .await
        .expect("effective");
    eprintln!("granted_tools = {:?}", ep.granted_tools);
    assert!(ep.granted_tools.iter().any(|t| t == "datasource.list"));

    let menu: Vec<AllowedTool> = [
        "datasource.list",
        "datasource.test",
        "store.query",
        "query.save",
        "series.read",
        "dashboard.catalog",
        "dashboard.pin",
        "dashboard.save",
        "federation.query",
        "viz.query",
        "flows.create",
    ]
    .iter()
    .map(|n| AllowedTool {
        name: n.to_string(),
        description: String::new(),
        input_schema: None,
    })
    .collect();
    let narrowed = narrow_tools(&menu, &ep.granted_tools);
    let names: Vec<&str> = narrowed.iter().map(|t| t.name.as_str()).collect();
    eprintln!("narrowed = {names:?}");
    for want in [
        "datasource.list",
        "store.query",
        "series.read",
        "viz.query",
        "dashboard.save",
        "federation.query",
    ] {
        assert!(names.contains(&want), "missing {want}");
    }
    assert!(!names.contains(&"flows.create"));
}
