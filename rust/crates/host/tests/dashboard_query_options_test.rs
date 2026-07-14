//! The P1 opener's regression pin (viz grafana-parity-backend scope): a UI-shaped cell carrying
//! top-level `queryOptions {maxDataPoints, minInterval, relativeTime, timeFrom, timeShift,
//! hideTimeOverride}` must survive the REAL `dashboard.save` → `dashboard.get` path — the exact MCP
//! entry (`call_dashboard_tool`) over a real mem:// store, no mocks. Before P1 the closed `Cell`
//! struct had no such field and no serde catch-all, so serde silently dropped it at the tool
//! boundary (`typed_arg::<Vec<Cell>>`); this test would have caught it.
//!
//! Also pins the other P1 additive fields end-to-end (`transparent`, `links` on the cell;
//! `timezone` on the dashboard; `description`/`skipUrlSync`/`allowCustomValue` on a variable) and
//! the additive guard: a v1 cell without any of them still saves + reads unchanged.

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

/// THE headline pin: the editor-parity UI's `queryOptions` trio (+ the P1 time-override trio)
/// survives save → get on the real MCP path. Before P1, serde dropped the whole object silently.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ui_shaped_query_options_survive_save_get() {
    let ws = "ws-qopts";
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
                "i": "c1", "x": 0, "y": 0, "w": 6, "h": 4, "v": 3,
                "view": "timeseries",
                "sources": [{ "refId": "A", "tool": "series.read", "args": { "series": "cooler.temp" } }],
                // The UI-shaped block, exactly as the editor sends it (camelCase, partial).
                "queryOptions": {
                    "maxDataPoints": 300,
                    "minInterval": "10s",
                    "relativeTime": "1h",
                    "timeFrom": "6h",
                    "timeShift": "1d",
                    "hideTimeOverride": true
                }
            }]
        }),
    )
    .await
    .expect("save succeeds");
    assert_eq!(
        saved["cells"][0]["queryOptions"]["maxDataPoints"], 300,
        "save's own return already carries queryOptions"
    );

    let got = call_dashboard_tool(&store, &ada, ws, "dashboard.get", &json!({ "id": "ops" }))
        .await
        .expect("get succeeds");
    let qo = &got["cells"][0]["queryOptions"];
    assert_eq!(qo["maxDataPoints"], 300, "maxDataPoints survived the store");
    assert_eq!(qo["minInterval"], "10s");
    assert_eq!(qo["relativeTime"], "1h");
    assert_eq!(qo["timeFrom"], "6h");
    assert_eq!(qo["timeShift"], "1d");
    assert_eq!(qo["hideTimeOverride"], true);
}

/// The remaining P1 additive fields ride the same real path: cell `transparent` + `links`,
/// dashboard `timezone`, variable `description`/`skipUrlSync`/`allowCustomValue`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn p1_additive_fields_survive_save_get() {
    let ws = "ws-p1-fields";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws);

    call_dashboard_tool(
        &store,
        &ada,
        ws,
        "dashboard.save",
        &json!({
            "id": "imp", "title": "Imported", "now": 10,
            "timezone": "Australia/Sydney",
            "cells": [{
                "i": "c1", "x": 0, "y": 0, "w": 6, "h": 4, "v": 3,
                "view": "stat",
                "transparent": true,
                "links": [{ "title": "Runbook", "url": "https://example.com/rb" }]
            }],
            "variables": [{
                "name": "region", "type": "custom", "custom": ["west", "east"],
                "description": "Deployment region",
                "skipUrlSync": true,
                "allowCustomValue": true
            }]
        }),
    )
    .await
    .expect("save succeeds");

    let got = call_dashboard_tool(&store, &ada, ws, "dashboard.get", &json!({ "id": "imp" }))
        .await
        .expect("get succeeds");
    assert_eq!(got["timezone"], "Australia/Sydney");
    let c = &got["cells"][0];
    assert_eq!(c["transparent"], true);
    assert_eq!(c["links"][0]["title"], "Runbook");
    let v = &got["variables"][0];
    assert_eq!(v["description"], "Deployment region");
    assert_eq!(v["skipUrlSync"], true);
    assert_eq!(v["allowCustomValue"], true);
}

/// The additive guard on the real path: a v1 cell (no v2/v3 fields, none of the P1 fields) and a
/// pre-P1 dashboard/variable shape still save + read unchanged — the new fields default cleanly and
/// stay off the wire where their skip predicates say so.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pre_p1_shapes_still_round_trip() {
    let ws = "ws-p1-guard";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws);

    call_dashboard_tool(
        &store,
        &ada,
        ws,
        "dashboard.save",
        &json!({
            "id": "old", "title": "Old", "now": 10,
            "cells": [{
                "i": "c1", "x": 0, "y": 0, "w": 4, "h": 3,
                "widget_type": "chart",
                "binding": { "series": "cooler.temp" }
            }],
            "variables": [{ "name": "env", "type": "custom", "custom": ["prod"] }]
        }),
    )
    .await
    .expect("v1 save succeeds");

    let got = call_dashboard_tool(&store, &ada, ws, "dashboard.get", &json!({ "id": "old" }))
        .await
        .expect("get succeeds");
    let c = &got["cells"][0];
    assert_eq!(c["widget_type"], "chart");
    assert_eq!(c["binding"]["series"], "cooler.temp");
    // The new fields defaulted; an absent queryOptions stays absent on the wire (skip-if-default),
    // so a pre-P1 record round-trips byte-stable rather than growing noise.
    assert!(
        c.get("queryOptions").is_none(),
        "empty queryOptions stays off the wire"
    );
    assert_eq!(got["variables"][0]["name"], "env");
}
