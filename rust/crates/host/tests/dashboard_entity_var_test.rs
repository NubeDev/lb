//! Regression pin (entity-data-plane scope, Phase D + generated-product-ux scope, Plane 1): an
//! `entity`-type dashboard variable carries a `entity` BINDING that the client's `entityVar.ts`
//! compiles its option resolver from. That binding must survive the REAL `dashboard.save` →
//! `dashboard.get` path — the exact MCP entry (`call_dashboard_tool`) over a real mem:// store, no
//! mocks. Before this field the closed `Variable` struct had no `entity` slot and no serde catch-all,
//! so `typed_arg::<Vec<Variable>>` silently DROPPED it at the tool boundary — an entity var then
//! resolved no options and a meter/site template dashboard rendered empty. The same silent-drop class
//! as `queryOptions`/`argsTemplate` before their fields landed; an in-memory unit test stays green
//! while the feature is dead, so the pin has to cross the wire.

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

/// THE pin: an entity var's binding (over the `meter` store table) survives save → get on the real
/// MCP path, so the client can compile its resolver and populate the dropdown. Before this field,
/// serde dropped the whole `entity` object silently and the var had no options.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn entity_var_binding_survives_save_get() {
    let ws = "ws-entity-var";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws);

    let saved = call_dashboard_tool(
        &store,
        &ada,
        ws,
        "dashboard.save",
        &json!({
            "id": "ems-meter-detail", "title": "Meter Detail", "now": 10,
            "variables": [{
                "name": "meter",
                "label": "Meter",
                "type": "entity",
                "required": true,
                "entity": {
                    "entity": "meter", "source": "ems-readings", "table": "meter",
                    "pk": "id", "display": "name", "backend": "store"
                }
            }],
            "cells": [{
                "i": "points", "x": 0, "y": 0, "w": 6, "h": 6, "v": 3,
                "view": "table",
                // A cell that interpolates the entity var against a store read of the meter's points.
                "sources": [{
                    "refId": "A", "tool": "store.query",
                    "args": { "sql": "SELECT data.name AS name FROM point WHERE data.meter_id = ${meter:sqlstring}" }
                }]
            }]
        }),
    )
    .await
    .expect("save succeeds");
    // The save's own return already carries the binding (mirrors dashboard.get).
    assert_eq!(
        saved["variables"][0]["entity"]["table"], "meter",
        "save's own return carries the entity binding"
    );

    let got = call_dashboard_tool(
        &store,
        &ada,
        ws,
        "dashboard.get",
        &json!({ "id": "ems-meter-detail" }),
    )
    .await
    .expect("get succeeds");

    let v = &got["variables"][0];
    assert_eq!(v["type"], "entity");
    assert_eq!(v["required"], true);
    let binding = &v["entity"];
    assert_eq!(binding["entity"], "meter", "entity id survived the store");
    assert_eq!(binding["table"], "meter");
    assert_eq!(binding["pk"], "id");
    assert_eq!(binding["display"], "name");
    assert_eq!(binding["backend"], "store");
    assert_eq!(binding["source"], "ems-readings");
    // The interpolation slot rode through untouched (the client resolves it at render).
    assert_eq!(
        got["cells"][0]["sources"][0]["args"]["sql"],
        "SELECT data.name AS name FROM point WHERE data.meter_id = ${meter:sqlstring}"
    );
}

/// The additive guard on the real path: a variable with NO entity binding (a plain `custom` var)
/// still saves + reads unchanged, and `entity` stays off the wire — a pre-entity dashboard
/// round-trips byte-clean rather than growing an `"entity": null` on every variable.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn non_entity_var_keeps_entity_off_the_wire() {
    let ws = "ws-entity-guard";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws);

    call_dashboard_tool(
        &store,
        &ada,
        ws,
        "dashboard.save",
        &json!({
            "id": "plain", "title": "Plain", "now": 10,
            "cells": [],
            "variables": [{ "name": "env", "type": "custom", "custom": ["prod", "staging"] }]
        }),
    )
    .await
    .expect("save succeeds");

    let got = call_dashboard_tool(&store, &ada, ws, "dashboard.get", &json!({ "id": "plain" }))
        .await
        .expect("get succeeds");
    let v = &got["variables"][0];
    assert_eq!(v["name"], "env");
    assert!(
        v.get("entity").is_none(),
        "a non-entity variable carries no entity key on the wire"
    );
}
