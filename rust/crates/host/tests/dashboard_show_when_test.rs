//! Conditional targets + conditional variables must survive the REAL `dashboard.save` →
//! `dashboard.get` path (`call_dashboard_tool` over a mem:// store, no mocks).
//!
//! The regression this pins was found in the field: an authored report — three comparison baselines
//! per panel (previous period, a chosen site, the estate average) gated by one `comparison`
//! variable, and the pickers for those baselines gated on the same variable — was imported into a
//! node and came back with every gate gone. `Target` and `Variable` had no `show_when` field and no
//! serde catch-all, so the expressions were dropped at the tool boundary and the board drew all
//! four baselines at once with a `Comparison` control that did nothing. The client evaluates these
//! (a target's resolves to a plain `hide` before dispatch) — the host only has to carry them, which
//! is exactly what it was failing to do.

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

/// THE headline pin: a panel's alternative baselines and the pickers that select between them keep
/// their expressions across save → get.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn conditional_targets_and_variables_survive_save_get() {
    let ws = "ws-showwhen";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws);

    call_dashboard_tool(
        &store,
        &test,
        ws,
        "dashboard.save",
        &json!({
            "id": "report", "title": "Report", "now": 10,
            "cells": [{
                "i": "c1", "x": 0, "y": 0, "w": 12, "h": 6, "v": 3,
                "view": "timeseries",
                "sources": [
                    { "refId": "A", "tool": "federation.query", "args": { "sql": "current" } },
                    { "refId": "B", "tool": "federation.query", "args": { "sql": "previous" },
                      "showWhen": "${comparison} == Previous Period" },
                    { "refId": "C", "tool": "federation.query", "args": { "sql": "other site" },
                      "showWhen": "${comparison} == Site" }
                ]
            }],
            "variables": [
                { "name": "comparison", "type": "custom" },
                { "name": "comparisonSite", "type": "custom", "showWhen": "${comparison} == Site" }
            ]
        }),
    )
    .await
    .expect("save");

    let got = call_dashboard_tool(&store, &test, ws, "dashboard.get", &json!({ "id": "report" }))
        .await
        .expect("get");

    let sources = got["cells"][0]["sources"].as_array().expect("sources");
    assert_eq!(sources.len(), 3);
    assert_eq!(sources[1]["showWhen"], "${comparison} == Previous Period");
    assert_eq!(sources[2]["showWhen"], "${comparison} == Site");
    assert_eq!(got["variables"][1]["showWhen"], "${comparison} == Site");
}

/// The additive guard, both halves: an UNGATED target/variable round-trips with no `showWhen` key at
/// all (the field is skip-if-empty), so a board that predates conditionals is byte-stable.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_ungated_target_carries_no_show_when_key() {
    let ws = "ws-showwhen-additive";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws);

    call_dashboard_tool(
        &store,
        &test,
        ws,
        "dashboard.save",
        &json!({
            "id": "plain", "title": "Plain", "now": 10,
            "cells": [{
                "i": "c1", "x": 0, "y": 0, "w": 6, "h": 4, "v": 3, "view": "stat",
                "sources": [{ "refId": "A", "tool": "series.read", "args": { "series": "t" } }]
            }],
            "variables": [{ "name": "site", "type": "custom" }]
        }),
    )
    .await
    .expect("save");

    let got = call_dashboard_tool(&store, &test, ws, "dashboard.get", &json!({ "id": "plain" }))
        .await
        .expect("get");

    assert!(got["cells"][0]["sources"][0].get("showWhen").is_none());
    assert!(got["variables"][0].get("showWhen").is_none());
}
