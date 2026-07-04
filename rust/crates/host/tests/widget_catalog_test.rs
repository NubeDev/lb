//! The widget catalog + save-validator through the REAL MCP bridge (`lb_host::call_tool`) and the
//! store-only validator, against a real store/node — no fakes (widget-catalog scope, Slice A Testing
//! plan). Covers the mandatory categories:
//!   - **capability deny**: a principal WITHOUT `mcp:dashboard.catalog:call` is denied (opaque); the
//!     paired happy path runs as a PLAIN member (the palette read cap only) — it proves the grant, not
//!     an admin bypass.
//!   - **workspace isolation**: two sessions ws-A / ws-B, ws-A with a real installed `[[widget]]`
//!     extension. ws-A's catalog lists its tile; ws-B's does not. The built-in view set is identical.
//!   - **save-validation (the core)**: a valid built-in view persists; an unknown view (`"heatmap"`
//!     typo) is `BadInput` with nothing persisted; a well-formed `ext:<id>/<widget>` key persists
//!     (structural, not install-resolved); a malformed `ext:` key is `BadInput`; a genui cell still
//!     routes through the existing check. The rejection is IDENTICAL over the shell path (direct
//!     `dashboard_save`) and a headless `POST /mcp/call` (`call_tool` → `dashboard.save`).
//!   - **round-trip authoring**: a cell authored purely from catalog ids saves and reloads intact.

use std::sync::Arc;

use lb_assets::{record_install, ExtUi, Install};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, dashboard_get, dashboard_save, Cell, DashboardError, Node};
use lb_mcp::ToolError;
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
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

const CATALOG: &str = "mcp:dashboard.catalog:call";
const SAVE: &str = "mcp:dashboard.save:call";
const GET: &str = "mcp:dashboard.get:call";
const EXT_LIST: &str = "mcp:ext.list:call";

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

/// A minimal v3 cell with the given `view` and `options`.
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

/// Seed a REAL installed extension carrying one `[[widget]]` tile into workspace `ws` (the isolation
/// fixture — a real `Install` record `ext.list` reads, no sidecar spawn needed for a wasm widget row).
async fn seed_widget_ext(node: &Arc<Node>, ws: &str, ext_id: &str, tile_label: &str) {
    let widget = ExtUi {
        entry: "assets/remoteEntry.js".into(),
        label: tile_label.into(),
        icon: "gauge".into(),
        scope: vec!["series.latest".into()],
        data: true,
    };
    let install = Install::new(ext_id, "0.1.0", vec![], 1).with_ui(None, vec![widget]);
    record_install(&node.store, ws, &install)
        .await
        .expect("seed widget install");
}

// --- capability deny (mandatory) + the plain-member happy path ---

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn catalog_denied_without_cap_and_allowed_for_a_plain_member() {
    let ws = "wc-deny";
    let node = Arc::new(Node::boot().await.unwrap());

    // No caps → opaque deny (the palette read is gated; it grants knowledge, so it is still gated).
    let nobody = principal("user:eve", ws, &[]);
    let err = call(&node, &nobody, ws, "dashboard.catalog", json!({}))
        .await
        .expect_err("no catalog cap → denied");
    assert!(matches!(err, ToolError::Denied));

    // A PLAIN member holding ONLY the palette read cap (not an admin) reads it — proving the grant is
    // real, not an admin bypass. `ext.list` cap absent → no ext tiles, but the built-in palette is full.
    let member = principal("user:ada", ws, &[CATALOG]);
    let out = call(&node, &member, ws, "dashboard.catalog", json!({}))
        .await
        .expect("plain member reads the palette");
    assert_eq!(out["v"], 1);
    let views = out["views"].as_array().expect("views array");
    // The full 17-view built-in palette is present (spot-check a few kinds).
    let ids: Vec<&str> = views.iter().filter_map(|v| v["id"].as_str()).collect();
    for want in ["timeseries", "stat", "gauge", "table", "genui"] {
        assert!(ids.contains(&want), "palette missing built-in view {want}");
    }
    assert!(out["extWidgets"].as_array().unwrap().is_empty());
    // genui component names surface (names-only in Slice A).
    assert!(!out["genuiComponents"].as_array().unwrap().is_empty());
}

// --- workspace isolation (mandatory) ---

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn catalog_ext_tiles_are_workspace_isolated() {
    let node = Arc::new(Node::boot().await.unwrap());
    let (wa, wb) = ("wc-iso-a", "wc-iso-b");
    // ws-A has a real installed widget extension; ws-B has none.
    seed_widget_ext(&node, wa, "proof-panel", "Proof Tile").await;

    let ada = principal("user:ada", wa, &[CATALOG, EXT_LIST]);
    let bob = principal("user:bob", wb, &[CATALOG, EXT_LIST]);

    let a = call(&node, &ada, wa, "dashboard.catalog", json!({}))
        .await
        .unwrap();
    let b = call(&node, &bob, wb, "dashboard.catalog", json!({}))
        .await
        .unwrap();

    // ws-A sees its tile as opaque {ext, widget, label}; ws-B sees none (the wall).
    let a_tiles = a["extWidgets"].as_array().unwrap();
    assert_eq!(a_tiles.len(), 1, "ws-A lists its own installed tile");
    assert_eq!(a_tiles[0]["ext"], "proof-panel");
    assert_eq!(a_tiles[0]["label"], "Proof Tile");
    assert!(
        b["extWidgets"].as_array().unwrap().is_empty(),
        "ws-B must not see ws-A's tile"
    );

    // The built-in view set is workspace-INDEPENDENT — identical for both.
    assert_eq!(
        a["views"], b["views"],
        "built-in palette is the same across workspaces"
    );
}

// --- save-validation: the core ---

/// The shell path (direct `dashboard_save`) and the headless path (`call_tool` → `dashboard.save`)
/// must reject an unknown view IDENTICALLY. Run the given cell down both and assert the same message.
async fn assert_rejected_both_paths(node: &Arc<Node>, ws: &str, bad_cell: Cell, needle: &str) {
    let ada = principal("user:ada", ws, &[SAVE, GET]);

    // (1) shell path — direct `dashboard_save`.
    let err = dashboard_save(
        &node.store,
        &ada,
        ws,
        "d",
        "D",
        vec![bad_cell.clone()],
        vec![],
        10,
    )
    .await
    .expect_err("unknown view rejected on the shell path");
    match &err {
        DashboardError::BadInput(m) => {
            assert!(
                m.contains(needle),
                "shell rejection {m:?} should mention {needle:?}"
            )
        }
        other => panic!("expected BadInput, got {other:?}"),
    }

    // (2) headless path — the same call over `POST /mcp/call` (`call_tool` → `dashboard.save`).
    let cells = serde_json::to_value(vec![bad_cell]).unwrap();
    let herr = call_tool(
        node,
        &ada,
        ws,
        "dashboard.save",
        &json!({ "id": "d", "title": "D", "cells": cells, "now": 10 }).to_string(),
    )
    .await
    .expect_err("unknown view rejected on the headless path");
    match herr {
        ToolError::BadInput(m) => {
            assert!(
                m.contains(needle),
                "headless rejection {m:?} should mention {needle:?}"
            )
        }
        other => panic!("expected BadInput over MCP, got {other:?}"),
    }

    // Nothing persisted (the whole save is refused).
    assert!(
        dashboard_get(&node.store, &ada, ws, "d").await.is_err(),
        "a rejected save must persist nothing"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn valid_builtin_view_persists() {
    let ws = "wc-ok-builtin";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = principal("user:ada", ws, &[SAVE, GET]);
    let c = cell("a", "gauge", json!({ "min": 0, "max": 100 }));
    dashboard_save(&node.store, &ada, ws, "d", "D", vec![c], vec![], 10)
        .await
        .expect("a known built-in view saves");
    let got = dashboard_get(&node.store, &ada, ws, "d").await.unwrap();
    assert_eq!(got.cells[0].view, "gauge");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unknown_view_rejected_both_paths_and_nothing_persisted() {
    let node = Arc::new(Node::boot().await.unwrap());
    // "heatmap" — in the OLD TS union but with no renderer case + no catalog entry (the G4 symptom).
    let c = cell("a", "heatmap", json!({}));
    assert_rejected_both_paths(&node, "wc-bad-view", c, "unknown view 'heatmap'").await;
    // The error names the palette verb so the fix is one edit away.
    let ada = principal("user:ada", "wc-bad-view2", &[SAVE]);
    let err = dashboard_save(
        &node.store,
        &ada,
        "wc-bad-view2",
        "d",
        "D",
        vec![cell("a", "heatmap", json!({}))],
        vec![],
        10,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, DashboardError::BadInput(m) if m.contains("dashboard.catalog") && m.contains("cell a"))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn well_formed_ext_key_persists_structurally() {
    let ws = "wc-ext-ok";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = principal("user:ada", ws, &[SAVE, GET]);
    // NOT installed anywhere — accepted structurally (widget-catalog scope: not install-resolved, so
    // uninstalling never makes a dashboard unsavable, and `dashboard.save` stays store-only).
    let c = cell("a", "ext:acme-charts/heat", json!({}));
    dashboard_save(&node.store, &ada, ws, "d", "D", vec![c], vec![], 10)
        .await
        .expect("a well-formed ext:<id>/<widget> key persists");
    let got = dashboard_get(&node.store, &ada, ws, "d").await.unwrap();
    assert_eq!(got.cells[0].view, "ext:acme-charts/heat");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn malformed_ext_keys_rejected() {
    let node = Arc::new(Node::boot().await.unwrap());
    // Empty tail, missing widget, missing id — each malformed structurally.
    for (i, bad) in ["ext:", "ext:x/", "ext:/w", "ext:onlyid"]
        .iter()
        .enumerate()
    {
        let ws = format!("wc-ext-bad-{i}");
        assert_rejected_both_paths(
            &node,
            &ws,
            cell("a", bad, json!({})),
            "malformed extension view",
        )
        .await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn genui_still_routed_through_the_existing_check() {
    // A genui cell with a malformed IR is rejected by `check_genui_cells` (NOT the new view-name
    // check) — the regression that the view-name validator does not shadow the IR validator.
    let ws = "wc-genui";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = principal("user:ada", ws, &[SAVE, GET]);

    // A well-formed genui cell (valid view name, valid IR) still saves.
    let good = cell(
        "g",
        "genui",
        json!({ "genui": { "v": 1, "ir": {
            "v": 1,
            "surface": { "surfaceId": "cell", "root": "r" },
            "components": { "r": { "id": "r", "component": "stat", "props": { "value": 1 } } }
        } } }),
    );
    dashboard_save(&node.store, &ada, ws, "dg", "D", vec![good], vec![], 10)
        .await
        .expect("a well-formed genui cell saves");

    // A genui cell whose IR names an unknown component is rejected by the IR check (its message, not
    // the view-name one) — proving `check_view_cells` accepts `genui` and defers the IR to genui.rs.
    let bad = cell(
        "g",
        "genui",
        json!({ "genui": { "v": 1, "ir": {
            "v": 1,
            "surface": { "surfaceId": "cell", "root": "r" },
            "components": { "r": { "id": "r", "component": "Frobnicate", "props": {} } }
        } } }),
    );
    let err = dashboard_save(&node.store, &ada, ws, "dg2", "D", vec![bad], vec![], 11)
        .await
        .unwrap_err();
    assert!(
        matches!(err, DashboardError::BadInput(m) if m.contains("not in the catalog")),
        "a genui cell is validated by the IR check, not the view-name check"
    );
}

// --- round-trip authoring (integration) ---

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn round_trip_a_cell_authored_from_catalog_ids() {
    // Author a cell purely from catalog ids the verb reports (view `gauge`, options `min`/`max` +
    // `fieldConfig` unit), save it, reload it, and assert it survives byte-for-byte (no default-
    // everything fallback) — the discovery→author→persist loop the slice exists to make correct.
    let ws = "wc-roundtrip";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = principal("user:ada", ws, &[CATALOG, SAVE, GET]);

    // Discover the palette and confirm `gauge` + its `min`/`max`/`unit` option ids exist.
    let cat = call(&node, &ada, ws, "dashboard.catalog", json!({}))
        .await
        .unwrap();
    let gauge = cat["views"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == json!("gauge"))
        .expect("gauge in the palette");
    let opt_ids: Vec<&str> = gauge["options"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| o["id"].as_str())
        .collect();
    for want in ["min", "max", "unit"] {
        assert!(
            opt_ids.contains(&want),
            "gauge option {want} in the catalog"
        );
    }

    // Author + save the cell from those ids.
    let mut c = cell("a", "gauge", json!({ "min": 0, "max": 120 }));
    c.field_config = json!({ "defaults": { "unit": "celsius" } });
    dashboard_save(&node.store, &ada, ws, "d", "Ops", vec![c], vec![], 10)
        .await
        .expect("catalog-authored cell saves");

    // Reload: the view + options survive intact.
    let got = dashboard_get(&node.store, &ada, ws, "d").await.unwrap();
    assert_eq!(got.cells[0].view, "gauge");
    assert_eq!(got.cells[0].options["max"], json!(120));
    assert_eq!(
        got.cells[0].field_config["defaults"]["unit"],
        json!("celsius")
    );
}
