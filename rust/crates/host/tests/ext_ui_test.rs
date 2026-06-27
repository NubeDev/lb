//! Extension UI contributions at the host layer (ui-federation + dashboard-widgets scopes): installing
//! an extension that declares `[ui]`/`[widget]` **persists the contribution on the install** (scope
//! narrowed to the grant) and **`ext.list` surfaces it**, so the shell can build a cap-gated nav slot /
//! widget palette entry. Plus the load-bearing security case: the host-mediated bridge (`call_tool`)
//! **denies an ungranted tool** — a page is a gated caller, never a trusted decider.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, ext_list, install_extension, Node};

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

/// A wasm manifest that declares BOTH a page and a widget, requesting two read-only series verbs.
const UI_MANIFEST: &str = r#"
[extension]
id = "hello-ui"
version = "0.1.0"

[runtime]
tier = "wasm"
world = "lazybones:ext/extension@0.1.0"
placement = "either"

[capabilities]
request = ["mcp:series.find:call", "mcp:series.latest:call"]

[ui]
entry = "entry.mjs"
label = "Hello UI"
icon = "puzzle"
scope = ["series.find", "series.latest"]

[widget]
entry = "widget.mjs"
label = "Latest"
scope = ["series.latest"]

[visibility]
class = "public"
"#;

/// Load the real `hello` wasm so `install_extension` (which loads the component) has a component to
/// bring online. The UI contribution is independent of the runtime half.
fn hello_wasm() -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm");
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("read {}: {e} (build the hello ext first)", path.display()))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn install_persists_ui_and_widget_then_ext_list_surfaces_them() {
    let node = Node::boot().await.unwrap();
    let ws = "acme";
    let approved = vec![
        "mcp:series.find:call".to_string(),
        "mcp:series.latest:call".to_string(),
    ];
    install_extension(&node, ws, UI_MANIFEST, &hello_wasm(), &approved, 1)
        .await
        .expect("install with [ui]/[widget]");

    let admin = principal("user:ada", ws, &["mcp:ext.list:call"]);
    let rows = ext_list(&node, &admin, ws).await.expect("list");
    let row = rows
        .iter()
        .find(|r| r.ext == "hello-ui")
        .expect("row present");

    let ui = row.ui.as_ref().expect("page contribution surfaced");
    assert_eq!(ui.entry, "entry.mjs");
    assert_eq!(ui.label, "Hello UI");
    assert_eq!(
        ui.scope,
        vec!["series.find".to_string(), "series.latest".to_string()]
    );

    let widget = row.widget.as_ref().expect("widget contribution surfaced");
    assert_eq!(widget.entry, "widget.mjs");
    assert_eq!(widget.scope, vec!["series.latest".to_string()]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ui_scope_is_narrowed_to_the_grant() {
    // The manifest asks for series.find + series.latest, but the admin approves ONLY series.latest.
    // The persisted page scope must drop series.find — a page can never claim an ungranted tool.
    let node = Node::boot().await.unwrap();
    let ws = "acme";
    let approved = vec!["mcp:series.latest:call".to_string()];
    install_extension(&node, ws, UI_MANIFEST, &hello_wasm(), &approved, 1)
        .await
        .expect("install");

    let admin = principal("user:ada", ws, &["mcp:ext.list:call"]);
    let rows = ext_list(&node, &admin, ws).await.unwrap();
    let ui = rows
        .iter()
        .find(|r| r.ext == "hello-ui")
        .unwrap()
        .ui
        .clone()
        .unwrap();
    assert_eq!(
        ui.scope,
        vec!["series.latest".to_string()],
        "find dropped — not granted"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn bridge_denies_an_ungranted_tool() {
    // The bridge endpoint (`call_tool`) re-checks the capability: a page whose principal lacks the cap
    // is denied server-side, regardless of what its bundle posts. This is the load-bearing guarantee.
    let node = Node::boot().await.unwrap();
    let ws = "acme";
    let ungranted = principal("user:page", ws, &[]); // holds NO caps
    let res = call_tool(&node, &ungranted, ws, "series.find", "{\"tags\":[]}").await;
    assert!(res.is_err(), "an ungranted bridged call must be denied");
}
