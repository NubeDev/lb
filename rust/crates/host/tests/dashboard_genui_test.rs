//! Host-side `view:"genui"` validation on `dashboard.save` (genui scope, Decision 6 — the ONE host
//! change in the slice). Proves the loud-rejection contract every writer (shell, `POST /mcp/call`,
//! routed Zenoh, external-agent) gets against a REAL store: a well-formed genui cell saves; a malformed
//! one (unknown component, oversized, bad/absent `v`, dangling root, non-object `ir`) is REFUSED at
//! write time (`DashboardError::BadInput`), not degraded at view time. An UN-AUTHORED draft (no `genui`
//! block, or one with no `ir` yet) is a legitimate savable draft, NOT a rejection. Plus the mandatory
//! categories: capability-DENY (no save cap) and workspace-ISOLATION for a genui dashboard.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{dashboard_get, dashboard_save, Cell, DashboardError};
use lb_store::Store;
use serde_json::{json, Value};

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

const SAVE: &str = "mcp:dashboard.save:call";

/// A `view:"genui"` cell carrying `options.genui = { v, ir }`.
fn genui_cell(options_genui: Value) -> Cell {
    Cell {
        i: "g1".into(),
        x: 0,
        y: 0,
        w: 6,
        h: 4,
        v: 3,
        widget_type: "genui".into(),
        title: String::new(),
        view: "genui".into(),
        binding: json!(null),
        source: Default::default(),
        action: Default::default(),
        options: json!({ "genui": options_genui }),
        description: String::new(),
        sources: Vec::new(),
        transformations: Vec::new(),
        field_config: json!(null),
        plugin_version: String::new(),
        panel_ref: String::new(),
        panel_vars: json!(null),
        panel_missing: false,
    }
}

/// A well-formed IR: one `stat` component as the root.
fn good_ir() -> Value {
    json!({
        "v": 1,
        "surface": { "surfaceId": "cell", "root": "r" },
        "components": {
            "r": { "id": "r", "component": "stat", "props": { "value": 42, "label": "Count" } }
        }
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn accepts_a_well_formed_genui_cell() {
    let ws = "ws-genui-ok";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, &[SAVE, "mcp:dashboard.get:call"]);
    let cell = genui_cell(json!({ "v": 1, "ir": good_ir() }));
    dashboard_save(&store, &ada, ws, "d", "D", vec![cell], vec![], 10)
        .await
        .expect("well-formed genui cell saves");
    let got = dashboard_get(&store, &ada, ws, "d").await.unwrap();
    assert_eq!(got.cells.len(), 1);
    assert_eq!(got.cells[0].view, "genui");
}

async fn assert_rejected(options_genui: Value, needle: &str) {
    let ws = "ws-genui-bad";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, &[SAVE]);
    let cell = genui_cell(options_genui);
    let err = dashboard_save(&store, &ada, ws, "d", "D", vec![cell], vec![], 10)
        .await
        .expect_err("malformed genui cell is rejected at save");
    match err {
        DashboardError::BadInput(m) => assert!(
            m.contains(needle),
            "rejection message {m:?} should mention {needle:?}"
        ),
        other => panic!("expected BadInput, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rejects_unknown_component() {
    let mut ir = good_ir();
    ir["components"]["r"]["component"] = json!("Frobnicate");
    assert_rejected(json!({ "v": 1, "ir": ir }), "not in the catalog").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rejects_absent_or_bad_version() {
    let mut ir = good_ir();
    ir["v"] = json!(0);
    assert_rejected(json!({ "v": 1, "ir": ir.clone() }), "unknown to this node").await;
    ir["v"] = json!(999);
    assert_rejected(json!({ "v": 1, "ir": ir }), "unknown to this node").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rejects_dangling_root() {
    let mut ir = good_ir();
    ir["surface"]["root"] = json!("ghost");
    assert_rejected(json!({ "v": 1, "ir": ir }), "root").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn allows_an_unauthored_draft() {
    // A `view:"genui"` cell the author just ADDED but hasn't generated an IR for yet is a legitimate
    // savable draft (no `genui` block, or one with no `ir`) — like a blank timeseries. It must NOT be
    // rejected; the view renders an "author me" placeholder. Only a present-but-malformed IR is refused.
    let ws = "ws-genui-draft";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, &[SAVE, "mcp:dashboard.get:call"]);

    // (a) no `genui` block at all.
    let mut c1 = genui_cell(json!({ "v": 1, "ir": good_ir() }));
    c1.options = json!({});
    dashboard_save(&store, &ada, ws, "d1", "D", vec![c1], vec![], 10)
        .await
        .expect("a genui cell with no options.genui is a savable draft");

    // (b) a `genui` block with no `ir` yet.
    let c2 = genui_cell(json!({ "v": 1 }));
    dashboard_save(&store, &ada, ws, "d2", "D", vec![c2], vec![], 11)
        .await
        .expect("a genui block with no ir is a savable draft");

    // (c) but a PRESENT, non-object `ir` is malformed → rejected. (A string that PARSES to a valid
    // IR object is normalized instead — see (d) — so only an unparseable string reaches this.)
    assert_rejected(
        json!({ "v": 1, "ir": "not-an-object" }),
        "must be a JSON object",
    )
    .await;

    // (d) lenient-args: a JSON-STRING ir that parses to a valid IR object saves (normalized to the
    // object the renderer expects) — the live-model stall was retrying exactly this shape.
    let c4 = genui_cell(json!({ "v": 1, "ir": good_ir().to_string() }));
    let saved = dashboard_save(&store, &ada, ws, "d4", "D", vec![c4], vec![], 12)
        .await
        .expect("a stringified-but-valid ir is normalized and saves");
    assert!(
        saved.cells[0].options["genui"]["ir"].is_object(),
        "the persisted ir must be the parsed object, not the string"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rejects_oversized_spec() {
    // A valid-but-huge IR (a giant literal prop) exceeds the ~8 KB bound.
    let mut ir = good_ir();
    ir["components"]["r"]["props"]["value"] = json!("x".repeat(9000));
    assert_rejected(json!({ "v": 1, "ir": ir }), "too large").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn deny_without_save_cap() {
    // The mandatory capability-deny: a principal WITHOUT the save cap cannot save a genui cell (the
    // authorize gate fires before validation — a denied writer never reaches the branch).
    let ws = "ws-genui-deny";
    let store = Store::memory().await.unwrap();
    let nobody = principal("user:mallory", ws, &[]); // no caps
    let cell = genui_cell(json!({ "v": 1, "ir": good_ir() }));
    let err = dashboard_save(&store, &nobody, ws, "d", "D", vec![cell], vec![], 10)
        .await
        .expect_err("no save cap → denied");
    assert!(matches!(err, DashboardError::Denied));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation_of_a_genui_dashboard() {
    // The mandatory workspace-isolation: a genui dashboard saved in W1 is invisible to a W2 principal.
    let store = Store::memory().await.unwrap();
    let w1 = "ws-genui-1";
    let w2 = "ws-genui-2";
    let ada = principal("user:ada", w1, &[SAVE]);
    let bob = principal("user:bob", w2, &[SAVE, "mcp:dashboard.get:call"]);
    let cell = genui_cell(json!({ "v": 1, "ir": good_ir() }));
    dashboard_save(&store, &ada, w1, "d", "D", vec![cell], vec![], 10)
        .await
        .unwrap();
    // Bob in W2 cannot see W1's dashboard (a fresh id in his workspace is absent).
    let got = dashboard_get(&store, &bob, w2, "d").await;
    assert!(
        got.is_err(),
        "W2 principal must not read the W1 genui dashboard"
    );
}
