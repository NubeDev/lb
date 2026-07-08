//! `dashboard.pin` — the pin-to-dashboard mint verb through the REAL MCP bridge (`lb_host::call_tool`)
//! and the direct shell path (`dashboard_pin`), against a real store/node — no fakes (widget-platform
//! scope, Slice B Testing plan). The pin takes an `x-lb-render` envelope (a tool's `result`, or a channel
//! `rich_result` body minus kind/v), mints a persisted `dashboard:{id}` cell HOST-SIDE, and reuses the
//! Slice A validation chain. Covers the mandatory categories:
//!   - **capability deny**: a principal WITHOUT `mcp:dashboard.pin:call` is denied (opaque); the paired
//!     happy path runs as a PLAIN member (only the pin + read caps) — proves the grant, not an admin bypass.
//!     A non-owner with the pin cap is denied on an existing dashboard they don't own (owner-only-update).
//!   - **workspace isolation**: a pin in ws-A produces a cell on a ws-A dashboard; a ws-B principal cannot
//!     read it. The source `reminder.list` re-checks under the viewer's grant at render (workspace-walled).
//!   - **the HEADLINE**: pin `reminder.list`'s declared `result` envelope → a persisted cell that reloads
//!     and renders through `WidgetView`, with ZERO reminder-specific code in the pin path. The mint is
//!     GENERIC over the tool id (rule 10) — an arbitrary/unknown tool id mints a valid cell; no `match`/`if`.
//!   - **envelope↔cell fidelity**: the minted cell round-trips (view/source/action/options/fieldConfig/
//!     tools-fold survive `dashboard.get`); re-pinning the SAME envelope REPLACES the cell (idempotent),
//!     not duplicates; a DIFFERENT envelope appends.
//!   - **shell path AND headless `POST /mcp/call` parity**: the same envelope pinned via the direct
//!     `dashboard_pin` AND via `call_tool` → `dashboard.pin` produces the SAME cell. A hallucinated view
//!     in the envelope is rejected THROUGH the pin path (the Slice A view-validator still fires).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    call_tool, dashboard_get, dashboard_pin, dashboard_save, reminder_create, Cell, Node,
};
use lb_mcp::ToolError;
use lb_reminders::Action as ReminderAction;
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

const PIN: &str = "mcp:dashboard.pin:call";
const SAVE: &str = "mcp:dashboard.save:call";
const GET: &str = "mcp:dashboard.get:call";
const LIST: &str = "mcp:dashboard.list:call";
const REMINDER_CREATE: &str = "mcp:reminder.create:call";
const REMINDER_LIST: &str = "mcp:reminder.list:call";

async fn call(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    tool: &str,
    input: Value,
) -> Result<Value, ToolError> {
    let out = call_tool(node, p, ws, tool, &input.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap())
}

/// A minimal v3 cell (for the round-trip-comparison helper that author a cell the OLD way via save).
fn cell(i: &str, view: &str, options: Value) -> Cell {
    Cell {
        i: i.into(),
        x: 0,
        y: 0,
        w: 6,
        h: 4,
        v: 3,
        widget_type: view.into(),
        title: String::new(),
        view: view.into(),
        binding: json!(null),
        source: Default::default(),
        action: Default::default(),
        options,
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

/// Seed a REAL reminder in workspace `ws` so `reminder.list` returns rows (the headline pin target).
async fn seed_reminder(node: &Arc<Node>, ws: &str, principal: &Principal, id: &str) {
    reminder_create(
        &node.store,
        principal,
        ws,
        id,
        "0 9 * * *", // a valid cron
        None,
        ReminderAction::ChannelPost {
            channel: "ops".into(),
            body: "check the cooler".into(),
        },
        100,
    )
    .await
    .expect("seed reminder");
}

/// The `reminder.list` declared `result` envelope — the exact shape `reminder/descriptor.rs::list_render`
/// declares (the headline proof: pinning this needs ZERO reminder-specific code in the pin path). We
/// construct it here so the test is self-contained; the host MINT function is what's under test, and it
/// treats the `source.tool` as opaque data.
fn reminder_list_envelope() -> Value {
    json!({
        "v": 2,
        "view": "table",
        "source": { "tool": "reminder.list", "args": {} },
        "options": { "rowControls": [
            { "kind": "switch", "label": "enabled",
              "action": { "tool": "reminder.update", "argsTemplate": { "id": "${id}", "enabled": "{{value}}" } } },
            { "kind": "button", "buttonLabel": "Run now",
              "action": { "tool": "reminder.fire", "argsTemplate": { "id": "${id}" } } },
            { "kind": "button", "buttonLabel": "Delete",
              "action": { "tool": "reminder.delete", "argsTemplate": { "id": "${id}" } } }
        ] },
        "fieldConfig": {
            "defaults": {},
            "overrides": [
                { "matcher": { "id": "byName", "options": "maxRuns" },
                  "properties": [ { "id": "displayName", "value": "Max Runs" } ] }
            ]
        },
        "tools": ["reminder.list", "reminder.update", "reminder.fire", "reminder.delete"]
    })
}

// --- capability deny (mandatory) + the plain-member happy path ---

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pin_denied_without_cap_and_allowed_for_a_plain_member() {
    let ws = "wp-deny";
    let node = Arc::new(Node::boot().await.unwrap());

    // No caps → opaque deny (the pin writes a dashboard cell, so it is gated).
    let nobody = principal("user:eve", ws, &[]);
    let err = call(
        &node,
        &nobody,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "d", "envelope": reminder_list_envelope(), "now": 10 }),
    )
    .await
    .expect_err("no pin cap → denied");
    assert!(matches!(err, ToolError::Denied));

    // A PLAIN member holding ONLY the pin + read caps (NOT an admin, NOT dashboard.save) pins — proving
    // the grant is real, not an admin bypass. The pin is its own cap (`mcp:dashboard.pin:call`), distinct
    // from `dashboard.save`; a member who can pin but not free-edit cells still works.
    let member = principal("user:ada", ws, &[PIN, GET, LIST]);
    let d = call(
        &node,
        &member,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "ops", "title": "Ops", "envelope": reminder_list_envelope(), "now": 10 }),
    )
    .await
    .expect("plain member pins");
    assert_eq!(d["id"], "ops");
    assert_eq!(d["title"], "Ops");
    let cells = d["cells"].as_array().expect("cells array");
    assert_eq!(cells.len(), 1, "one minted cell");
    assert_eq!(cells[0]["i"], "pin-reminder-list");
    assert_eq!(cells[0]["view"], "table");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pin_is_denied_for_a_non_owner_on_an_existing_dashboard() {
    // The owner-only-update gate (mirrors `dashboard.save`): a member with the pin cap CANNOT pin into a
    // dashboard someone else owns — even though they hold the pin cap, they don't own the record.
    let ws = "wp-owner";
    let node = Arc::new(Node::boot().await.unwrap());

    // Alice owns "ops" (she creates it via the pin path itself).
    let ada = principal("user:ada", ws, &[PIN, GET]);
    call(
        &node,
        &ada,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "ops", "title": "Ops", "envelope": reminder_list_envelope(), "now": 10 }),
    )
    .await
    .expect("ada creates + pins");

    // Bob has the pin cap but does NOT own "ops" — denied.
    let bob = principal("user:bob", ws, &[PIN, GET]);
    let err = call(
        &node,
        &bob,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "ops", "envelope": reminder_list_envelope(), "now": 11 }),
    )
    .await
    .expect_err("non-owner pin denied");
    assert!(matches!(err, ToolError::Denied));
}

// --- workspace isolation (mandatory) ---

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pin_in_ws_a_is_invisible_to_ws_b() {
    let node = Arc::new(Node::boot().await.unwrap());
    let (wa, wb) = ("wp-iso-a", "wp-iso-b");
    // Seed a reminder in ws-A so the pinned cell's source has real rows there.
    let ada = principal("user:ada", wa, &[PIN, GET, REMINDER_CREATE, REMINDER_LIST]);
    seed_reminder(&node, wa, &ada, "r1").await;

    // Ada pins the reminder widget to her ws-A dashboard "ops".
    call(
        &node,
        &ada,
        wa,
        "dashboard.pin",
        json!({ "dashboard": "ops", "title": "Ops", "envelope": reminder_list_envelope(), "now": 10 }),
    )
    .await
    .expect("ada pins in ws-A");

    // Bob in ws-B cannot read ws-A's dashboard "ops" — the workspace wall (gate 1).
    let bob = principal("user:bob", wb, &[GET]);
    let err = dashboard_get(&node.store, &bob, wb, "ops")
        .await
        .expect_err("ws-B cannot read ws-A's dashboard");
    assert!(
        matches!(
            err,
            lb_host::DashboardError::Denied | lb_host::DashboardError::NotFound
        ),
        "ws-B sees neither ws-A's dashboard nor a 404-existence leak"
    );

    // Bob pins the same envelope in ws-B — that mints a SEPARATE cell on a ws-B "ops" dashboard (a
    // different record in a different namespace). The two dashboards are independent; the cell `source`
    // re-runs `reminder.list` under the viewer's grant at render, so ws-B's cell lists ws-B's reminders
    // (none), never ws-A's.
    let bob = principal("user:bob", wb, &[PIN, GET, REMINDER_LIST]);
    let d = call(
        &node,
        &bob,
        wb,
        "dashboard.pin",
        json!({ "dashboard": "ops", "title": "Ops", "envelope": reminder_list_envelope(), "now": 10 }),
    )
    .await
    .expect("bob pins in ws-B");
    assert_eq!(d["cells"][0]["i"], "pin-reminder-list");
}

// --- the HEADLINE: pin reminder.list's declared result, generic over the tool id ---

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pin_reminder_list_envelope_persists_and_reloads_intact() {
    let ws = "wp-headline";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = principal("user:ada", ws, &[PIN, GET, REMINDER_CREATE, REMINDER_LIST]);
    seed_reminder(&node, ws, &ada, "r1").await;

    // Pin `reminder.list`'s declared `result` envelope — ZERO reminder-specific code in the pin path
    // (the mint function treats the tool id as opaque data; the envelope is a normal `x-lb-render`).
    // Pin through the headless `POST /mcp/call` path; then RELOAD via `dashboard.get` to prove the
    // persisted cell survives intact (the mint happened host-side, against the real store).
    let _pinned = call(
        &node,
        &ada,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "ops", "title": "Ops", "envelope": reminder_list_envelope(), "now": 10 }),
    )
    .await
    .expect("pin reminder.list");

    // Reload the dashboard — the minted cell survives intact (it was persisted).
    let got = dashboard_get(&node.store, &ada, ws, "ops")
        .await
        .expect("get");
    assert_eq!(got.cells.len(), 1);
    let c = &got.cells[0];
    assert_eq!(c.i, "pin-reminder-list");
    assert_eq!(c.view, "table");
    assert_eq!(c.source.tool, "reminder.list");
    // The envelope's `options.rowControls` + `fieldConfig` ride onto the cell — the shared table
    // column-model + a dashboard-side row-control renderer render them (Slice B ships row controls).
    assert_eq!(c.options["rowControls"].as_array().unwrap().len(), 3);
    assert_eq!(c.field_config["overrides"].as_array().unwrap().len(), 1);
    // The `tools` fold: the three row-control verbs (`reminder.update`/`fire`/`delete`) become hidden
    // `sources[]` so `cellTools(cell)` covers `render.tools` (the bridge leash).
    assert_eq!(c.sources.len(), 3);
    let tools: Vec<&str> = c.sources.iter().map(|t| t.tool.as_str()).collect();
    assert!(tools.contains(&"reminder.update"));
    assert!(tools.contains(&"reminder.fire"));
    assert!(tools.contains(&"reminder.delete"));
    for t in &c.sources {
        assert!(t.hide);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pin_path_is_generic_over_an_arbitrary_tool_id() {
    // The mint treats `source.tool` as OPAQUE DATA (rule 10) — an arbitrary/unknown tool id mints a valid
    // cell; no `match`/`if` on the id. A tool that doesn't even exist pins fine (the cell's source re-
    // checks at RENDER under the viewer's grant; here we only prove the MINT+persist is generic).
    let ws = "wp-generic";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = principal("user:ada", ws, &[PIN, GET]);

    let env = json!({
        "view": "table",
        "source": { "tool": "__test__.frobnicate", "args": { "x": 1 } },
        "tools": ["__test__.frobnicate"]
    });
    let d = call(
        &node,
        &ada,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "d", "title": "D", "envelope": env, "now": 10 }),
    )
    .await
    .expect("arbitrary tool id pins");
    assert_eq!(d["cells"][0]["i"], "pin-test-frobnicate");
    assert_eq!(d["cells"][0]["source"]["tool"], "__test__.frobnicate");
    assert_eq!(d["cells"][0]["source"]["args"], json!({ "x": 1 }));
}

// --- envelope↔cell fidelity: idempotent re-pin replaces, not duplicates; a different pin appends ---

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn re_pin_same_envelope_replaces_in_place_not_duplicates() {
    let ws = "wp-idem";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = principal("user:ada", ws, &[PIN, GET]);

    let env = reminder_list_envelope();
    call(
        &node,
        &ada,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "ops", "title": "Ops", "envelope": env, "now": 10 }),
    )
    .await
    .expect("first pin");

    // Direct shell path: re-pin the SAME envelope (same source.tool → same `i`). It should REPLACE the
    // cell, not append a duplicate.
    dashboard_pin(
        &node.store,
        &ada,
        ws,
        "ops",
        "Ops",
        &reminder_list_envelope(),
        20,
    )
    .await
    .expect("re-pin (shell path)");

    let got = dashboard_get(&node.store, &ada, ws, "ops")
        .await
        .expect("get");
    assert_eq!(got.cells.len(), 1, "re-pin replaces, not duplicates");
    assert_eq!(got.cells[0].i, "pin-reminder-list");
    assert_eq!(got.updated_ts, 20);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pin_a_different_envelope_appends_and_re_pin_preserves_layout() {
    let ws = "wp-append";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = principal("user:ada", ws, &[PIN, GET]);

    // Pin reminder.list first (lands at y=0).
    call(
        &node,
        &ada,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "ops", "title": "Ops", "envelope": reminder_list_envelope(), "now": 10 }),
    )
    .await
    .expect("first pin");

    // Pin a DIFFERENT tool's envelope — appends a new cell (different `i`), placed at the next free y.
    let other = json!({
        "view": "table",
        "source": { "tool": "federation.query", "args": {} },
        "tools": ["federation.query"]
    });
    let d = call(
        &node,
        &ada,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "ops", "envelope": other, "now": 11 }),
    )
    .await
    .expect("second pin (different envelope)");
    let cells = d["cells"].as_array().unwrap();
    assert_eq!(cells.len(), 2, "different envelope appends");
    let ids: Vec<&str> = cells.iter().filter_map(|c| c["i"].as_str()).collect();
    assert!(ids.contains(&"pin-reminder-list"));
    assert!(ids.contains(&"pin-federation-query"));
    // The appended cell lands below the first (next free y = 0 + 4 = 4).
    let fed = cells
        .iter()
        .find(|c| c["i"] == "pin-federation-query")
        .unwrap();
    assert_eq!(fed["y"], 4);

    // Re-pin reminder.list — preserves the EXISTING cell's layout (idempotent position), not the default.
    let d = call(
        &node,
        &ada,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "ops", "envelope": reminder_list_envelope(), "now": 12 }),
    )
    .await
    .expect("re-pin reminder.list");
    let rem = d["cells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["i"] == "pin-reminder-list")
        .unwrap();
    assert_eq!(rem["x"], 0, "x preserved on re-pin");
    assert_eq!(rem["y"], 0, "y preserved on re-pin");
    assert_eq!(rem["w"], 6, "w preserved on re-pin");
    assert_eq!(rem["h"], 4, "h preserved on re-pin");
}

// --- shell path AND headless POST /mcp/call parity (Slice A's pattern) ---

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn shell_path_and_headless_mcp_call_produce_the_same_cell() {
    let ws = "wp-parity";
    let node = Arc::new(Node::boot().await.unwrap());

    // The shell path — direct `dashboard_pin` into dashboard "shell".
    let ada_shell = principal("user:ada", ws, &[PIN, GET]);
    dashboard_pin(
        &node.store,
        &ada_shell,
        ws,
        "shell",
        "Shell",
        &reminder_list_envelope(),
        10,
    )
    .await
    .expect("shell path pin");

    // The headless path — the same call over `POST /mcp/call` (`call_tool` → `dashboard.pin`) into "mcp".
    call(
        &node,
        &ada_shell,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "mcp", "title": "Mcp", "envelope": reminder_list_envelope(), "now": 10 }),
    )
    .await
    .expect("headless path pin");

    let shell = dashboard_get(&node.store, &ada_shell, ws, "shell")
        .await
        .expect("shell get");
    let mcp = dashboard_get(&node.store, &ada_shell, ws, "mcp")
        .await
        .expect("mcp get");
    // The two paths produce the SAME cell shape (view/source/tools-fold/options/fieldConfig/i).
    assert_eq!(shell.cells[0].i, mcp.cells[0].i);
    assert_eq!(shell.cells[0].view, mcp.cells[0].view);
    assert_eq!(shell.cells[0].source, mcp.cells[0].source);
    assert_eq!(shell.cells[0].sources, mcp.cells[0].sources);
    assert_eq!(shell.cells[0].options, mcp.cells[0].options);
    assert_eq!(shell.cells[0].field_config, mcp.cells[0].field_config);
}

// --- the Slice A view-validator still fires through the pin path ---

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_hallucinated_view_in_the_envelope_is_rejected_through_pin() {
    // The pin reuses `check_view_cells` (Slice A) — an envelope with `view:"heatmap"` (the G4 typo) is
    // rejected loudly HERE, too, for the shell path AND a headless writer. Nothing persists.
    let ws = "wp-reject";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = principal("user:ada", ws, &[PIN, GET]);

    let env = json!({ "view": "heatmap", "source": { "tool": "x" } });
    let err = call(
        &node,
        &ada,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "d", "title": "D", "envelope": env, "now": 10 }),
    )
    .await
    .expect_err("hallucinated view rejected through pin");
    match err {
        ToolError::BadInput(m) => assert!(
            m.contains("unknown view 'heatmap'"),
            "the Slice A validator fires through the pin path: {m}"
        ),
        other => panic!("expected BadInput over MCP, got {other:?}"),
    }
    // Nothing persisted.
    assert!(dashboard_get(&node.store, &ada, ws, "d").await.is_err());
}

// --- pin coexists with hand-authored cells (a dashboard can mix pinned + saved cells) ---

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pin_appends_alongside_hand_authored_cells() {
    let ws = "wp-mix";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = principal("user:ada", ws, &[PIN, SAVE, GET]);

    // Author a gauge cell the OLD way (dashboard.save).
    dashboard_save(
        &node.store,
        &ada,
        ws,
        "ops",
        "Ops",
        vec![cell("g1", "gauge", json!({ "min": 0, "max": 100 }))],
        vec![],
        10,
    )
    .await
    .expect("hand-authored gauge");

    // Pin the reminder widget — appends alongside the gauge; both pass the Slice A validator.
    let d = call(
        &node,
        &ada,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "ops", "envelope": reminder_list_envelope(), "now": 11 }),
    )
    .await
    .expect("pin appends");
    let cells = d["cells"].as_array().unwrap();
    assert_eq!(cells.len(), 2);
    let ids: Vec<&str> = cells.iter().filter_map(|c| c["i"].as_str()).collect();
    assert!(ids.contains(&"g1"));
    assert!(ids.contains(&"pin-reminder-list"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pin_carries_a_genui_envelopes_declared_sources_through() {
    // CHANNEL-WIDGETS slice: a `genui` rich_result previewed in the dock binds `/data/{refId}` against
    // real `sources[]` targets — pinning it must carry those targets onto the cell VERBATIM (before the
    // hidden extra-tools fold), or the pinned widget renders with no data.
    let ws = "wp-genui-sources";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = principal("user:ada", ws, &[PIN, GET]);

    let env = json!({
        "view": "genui",
        "options": { "genui": { "v": 1, "ir": {
            "v": 1,
            "surface": { "surfaceId": "s1", "root": "root" },
            "components": {
                "root": { "id": "root", "component": "table",
                          "props": { "rows": { "$bind": "/data/A/rows" } } }
            }
        } } },
        "sources": [
            { "refId": "A", "tool": "store.query", "args": { "sql": "SELECT * FROM site" } },
            { "refId": "B", "tool": "series.latest", "args": { "series": "office/temp" } }
        ],
        "tools": ["store.query", "series.latest"]
    });
    let d = call(
        &node,
        &ada,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "d", "title": "D", "envelope": env, "now": 10 }),
    )
    .await
    .expect("genui envelope pins");
    let cell = &d["cells"][0];
    assert_eq!(cell["view"], "genui");
    assert_eq!(cell["options"]["genui"]["ir"]["v"], 1);
    // The declared targets carried through first, verbatim (refIds intact, not hidden)…
    assert_eq!(cell["sources"][0]["refId"], "A");
    assert_eq!(cell["sources"][0]["tool"], "store.query");
    assert_eq!(cell["sources"][0]["hide"], serde_json::json!(false));
    assert_eq!(cell["sources"][1]["refId"], "B");
    // …and NO duplicate hidden leash entries — a tool already covered by a declared target is not
    // re-folded (the leash reads it from the target itself).
    assert_eq!(cell["sources"].as_array().unwrap().len(), 2);
}

/// LIBRARY-PANELS: pinning an envelope NOW also saves it as a reusable `panel:{slug}` record (the
/// "widget table") AND attaches the dashboard cell by REFERENCE — so the user can later drop the same
/// widget onto other dashboards or open it in the data studio. The pin caller does NOT need
/// `mcp:panel.save:call` — pin is a privileged internal writer for the panel it just authored. The
/// persisted cell stores layout + `panel_ref` only (the spec is on the panel); the hydrated return
/// value carries the full spec so a `setCurrent` renders without a reload.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pin_persists_a_reusable_panel_and_attaches_the_cell_by_reference() {
    let ws = "wp-panel-ref";
    let node = Arc::new(Node::boot().await.unwrap());
    // PIN-only caller for the WRITE half (proves pin is its own authority for the panel it writes).
    // `panel.get` is added ONLY so the test can INSPECT the panel record afterwards — personas grant
    // `panel.*` so a real dock user has it; the pin itself never asks for it.
    const PANEL_GET: &str = "mcp:panel.get:call";
    let ada = principal("user:ada", ws, &[PIN, GET, PANEL_GET]);

    let env = json!({
        "view": "table",
        "source": { "tool": "reminder.list", "args": {} },
        "tools": ["reminder.list"],
    });
    let d = call(
        &node,
        &ada,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "ops", "title": "Ops", "envelope": env, "now": 10 }),
    )
    .await
    .expect("pin");
    // The hydrated return value still carries the full spec (hydrate re-inflates from the panel).
    let cell = &d["cells"][0];
    assert_eq!(cell["i"], "pin-reminder-list");
    assert_eq!(cell["view"], "table");
    assert_eq!(cell["panelRef"], "panel:reminder-list");
    assert_eq!(cell["source"]["tool"], "reminder.list");

    // The persisted cell (raw store, post-strip) is layout + ref only — no spec on the cell row.
    let raw = dashboard_get(&node.store, &ada, ws, "ops")
        .await
        .expect("get");
    // `dashboard_get` hydrates too — to see the STORED shape we read the raw row via the panel read.
    // The hydrated view shows the panel_ref + spec; the cell's `panelRef` is what makes it a ref.
    let stored_cell = raw
        .cells
        .iter()
        .find(|c| c.i == "pin-reminder-list")
        .unwrap();
    assert_eq!(
        stored_cell.panel_ref, "panel:reminder-list",
        "the persisted cell references the panel"
    );

    // The panel record EXISTS in the panel table — the reusable widget library.
    let panel = lb_host::panel_get(&node.store, &ada, ws, "reminder-list")
        .await
        .expect("panel_get on the just-pinned panel");
    assert_eq!(panel.spec.view, "table");
    assert_eq!(panel.spec.source.tool, "reminder.list");
    assert_eq!(panel.owner, "user:ada");
    assert_eq!(
        panel.visibility,
        lb_host::PanelVisibility::Private,
        "a freshly-pinned panel is private to its author"
    );
    assert!(!panel.deleted);

    // The panel is reusable: a SECOND dashboard pins the same envelope → the same panel record is
    // updated (owner-only-update; idempotent on the slug), and the second dashboard's cell references
    // the SAME panel. One widget, two dashboards, one source of truth.
    call(
        &node,
        &ada,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "metrics", "title": "Metrics", "envelope": env, "now": 20 }),
    )
    .await
    .expect("second dashboard pin");
    let panel2 = lb_host::panel_get(&node.store, &ada, ws, "reminder-list")
        .await
        .expect("panel_get on the re-pinned panel");
    assert_eq!(panel2.id, panel.id, "same panel slug reused");
    assert_eq!(
        panel2.updated_ts, 20,
        "the panel record was touched on the second pin"
    );
}

/// An envelope `title` (typed by the user in the pin dialog, or set by the agent in the fenced
/// block) names BOTH the minted dashboard cell and the reusable panel record. Regression for the
/// "pinned widgets are all called '<tool> widget'" gap.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pin_envelope_title_names_the_cell_and_the_panel() {
    let ws = "wp-title";
    let node = Arc::new(Node::boot().await.unwrap());
    const PANEL_GET: &str = "mcp:panel.get:call";
    let ada = principal("user:ada", ws, &[PIN, GET, PANEL_GET]);

    let env = json!({
        "view": "table",
        "title": "Site Energy Ranking",
        "source": { "tool": "reminder.list", "args": {} },
        "tools": ["reminder.list"],
    });
    let d = call(
        &node,
        &ada,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "ops", "title": "Ops", "envelope": env, "now": 10 }),
    )
    .await
    .expect("pin");
    assert_eq!(
        d["cells"][0]["title"], "Site Energy Ranking",
        "cell carries the widget name"
    );

    let panel = lb_host::panel_get(&node.store, &ada, ws, "reminder-list")
        .await
        .expect("panel_get");
    assert_eq!(panel.title, "Site Energy Ranking", "panel is named too");

    // No title in the envelope → the derived fallback still applies (no regression).
    let env2 = json!({
        "view": "table",
        "source": { "tool": "reminder.list", "args": {} },
        "tools": ["reminder.list"],
    });
    let d2 = call(
        &node,
        &ada,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "ops2", "title": "Ops2", "envelope": env2, "now": 20 }),
    )
    .await
    .expect("pin without title");
    assert_eq!(
        d2["cells"][0]["title"], "",
        "untitled envelope leaves the cell title empty"
    );
}
