//! Regression pin for the flow-bound-control wire bug (dashboard flow-control-argstemplate scope,
//! NubeIO/rubix-ai#25): a switch/slider bound to a flow node carries `action: { tool: "flows.inject",
//! argsTemplate: {...} }`, and that `argsTemplate` MUST survive the REAL `dashboard.save` →
//! `dashboard.get` path — the exact MCP entry (`call_dashboard_tool`) over a real mem:// store, no
//! mocks. Before the `#[serde(rename = "argsTemplate")]` on `Action.args_template`, the host didn't
//! recognise the camelCase key, stored `args_template: null`, and read it back `undefined` — the
//! binding evaporated in transit and the flow-fed-widgets feature read as entirely dead.
//!
//! The shipped UI unit tests (`controls.flow.test.tsx`) construct the `Action` in memory and never
//! cross the host wire, so they stayed green while the feature was broken. THIS test is the one that
//! would have caught it: it round-trips through the real verbs, and it is red before the rename.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::call_dashboard_tool;
use lb_store::Store;
use serde_json::json;

/// A principal `sub` in workspace `ws` holding the dashboard caps.
fn principal(sub: &str, ws: &str) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: vec![
            "mcp:dashboard.save:call".into(),
            "mcp:dashboard.get:call".into(),
        ],
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

/// THE headline pin: a flow-bound slider's `action.argsTemplate` (the `flows.inject` binding) survives
/// save → get on the real MCP path, and comes back under the camelCase wire key. Before the rename the
/// whole template was dropped to `null` at the tool boundary.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn flow_control_args_template_survives_save_get() {
    let ws = "ws-flowctl";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws);

    let saved = call_dashboard_tool(
        &store,
        &ada,
        ws,
        "dashboard.save",
        &json!({
            "id": "ops", "title": "Ops", "now": 10,
            "cells": [{
                "i": "c1", "x": 0, "y": 0, "w": 4, "h": 3, "v": 3,
                "view": "slider",
                // Exactly what the UI POSTs for a slider bound to a flow node's input port
                // (camelCase `argsTemplate`, the `{{value}}` slot the drag fills).
                "action": {
                    "tool": "flows.inject",
                    "argsTemplate": {
                        "id": "cooler-ctl",
                        "node": "setpoint-in",
                        "port": "payload",
                        "value": "{{value}}"
                    }
                }
            }]
        }),
    )
    .await
    .expect("save succeeds");
    // save's own return already carries the intact binding (not nulled).
    assert_eq!(
        saved["cells"][0]["action"]["argsTemplate"]["node"], "setpoint-in",
        "save's return carries the flow binding"
    );

    let got = call_dashboard_tool(&store, &ada, ws, "dashboard.get", &json!({ "id": "ops" }))
        .await
        .expect("get succeeds");
    let action = &got["cells"][0]["action"];
    assert_eq!(action["tool"], "flows.inject");
    // Mutation-proof: assert a specific INNER key, not merely non-null — a shim that stored `{}` would
    // pass a non-null check but still lose the binding.
    let tmpl = &action["argsTemplate"];
    assert_eq!(tmpl["id"], "cooler-ctl", "flow id survived the store");
    assert_eq!(tmpl["node"], "setpoint-in", "flow node survived the store");
    assert_eq!(tmpl["port"], "payload", "input port survived the store");
    assert_eq!(
        tmpl["value"], "{{value}}",
        "the value slot survived the store"
    );

    // The wire-shape guard: the key is `argsTemplate`, never the snake `args_template` (which nothing
    // in the platform emits or reads) — so a future refactor can't silently snake it again.
    assert!(
        action.get("args_template").is_none(),
        "the wire key is argsTemplate, not args_template"
    );
}

/// The additive guard on the real path: a read-only stat cell with NO `action` still saves + reads
/// unchanged — the flow-control rename touches only a control cell's binding, nothing else.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn non_control_cell_round_trips_without_action() {
    let ws = "ws-flowctl-guard";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws);

    call_dashboard_tool(
        &store,
        &ada,
        ws,
        "dashboard.save",
        &json!({
            "id": "read", "title": "Read", "now": 10,
            "cells": [{
                "i": "c1", "x": 0, "y": 0, "w": 6, "h": 4, "v": 3,
                "view": "timeseries",
                "sources": [{ "refId": "A", "tool": "series.read", "args": { "series": "cooler.temp" } }]
            }]
        }),
    )
    .await
    .expect("save succeeds");

    let got = call_dashboard_tool(&store, &ada, ws, "dashboard.get", &json!({ "id": "read" }))
        .await
        .expect("get succeeds");
    let c = &got["cells"][0];
    assert_eq!(c["view"], "timeseries");
    assert_eq!(c["sources"][0]["tool"], "series.read");
    // An empty action carries an empty tool + null template — no binding invented.
    assert_eq!(c["action"]["tool"], "");
}
