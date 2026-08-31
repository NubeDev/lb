//! `Dashboard.exportProfiles` — the board's saved, named `report.export` option sets
//! (report-pagination-and-export-options scope). Headless, real `mem://` store, real write path.
//!
//! **This field exists because the scope's "no stored export profiles" non-goal was wrong.** That
//! non-goal assumed the client could keep its own profiles; `Dashboard` has no serde catch-all and
//! DROPS unknown top-level keys, so a client-authored `exportProfiles` round-trips to nothing on the
//! very next layout save. There was no other place on the record for it to live — the same reason
//! `heading`, `reportIds`, `width` and `compact` are each typed. So the two claims worth pinning are
//! exactly those: it SURVIVES the real tool boundary with its option vocabulary intact, and it rides
//! the preserve-on-omit / empty-is-clear contract `reportIds` established.
//!
//! The host reads a profile NOWHERE — `report.export` takes no profile id, the client sends the
//! chosen profile's `options`. Nothing here asserts an export, on purpose.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    call_dashboard_tool, dashboard_get, dashboard_save, dashboard_save_meta, Cell, ExportOptions,
    ExportProfile, PageMeta,
};
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
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// A minimal cell, so a save is a real board rather than an empty record.
fn a_cell() -> Cell {
    Cell {
        i: "c1".into(),
        x: 0,
        y: 0,
        w: 12,
        h: 6,
        v: 3,
        view: "stat".into(),
        ..Cell::default()
    }
}

fn profile(id: &str, name: &str, options: ExportOptions) -> ExportProfile {
    ExportProfile {
        id: id.into(),
        name: name.into(),
        options,
    }
}

/// Preserve-on-omit and clear-on-empty, the identical contract `reportIds` carries.
///
/// Both directions matter to a real admin: a plain layout save (a panel drag) omits the key and must
/// not wipe the profiles they authored, while deleting the LAST profile sends `[]` and must actually
/// get the board back to the shipped default. "Omit to clear" would make the second impossible;
/// "empty preserves" would make it silently fail.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn export_profiles_preserve_on_omit() {
    let ws = "ws-profiles-omit";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws);

    // Absent on create ⇒ empty (no saved profiles), not a missing-field error.
    dashboard_save_meta(
        &store,
        &test,
        ws,
        "meters",
        "Meter Detail",
        PageMeta::default(),
        vec![a_cell()],
        vec![],
        10,
    )
    .await
    .unwrap();
    assert!(dashboard_get(&store, &test, ws, "meters")
        .await
        .unwrap()
        .export_profiles
        .is_empty());

    // Author two.
    let authored = vec![
        profile(
            "a3-landscape",
            "A3 landscape",
            ExportOptions {
                paper: "a3".into(),
                orientation: "landscape".into(),
                page_numbers: true,
                ..ExportOptions::default()
            },
        ),
        profile("shipped", "Default A4", ExportOptions::default()),
    ];
    dashboard_save_meta(
        &store,
        &test,
        ws,
        "meters",
        "Meter Detail",
        PageMeta {
            export_profiles: Some(authored.clone()),
            ..PageMeta::default()
        },
        vec![a_cell()],
        vec![],
        20,
    )
    .await
    .unwrap();
    assert_eq!(
        dashboard_get(&store, &test, ws, "meters")
            .await
            .unwrap()
            .export_profiles,
        authored
    );

    // A plain LAYOUT save sends no profiles — they must survive, or the first panel drag silently
    // deletes every profile the admin set up.
    dashboard_save(
        &store,
        &test,
        ws,
        "meters",
        "Meter Detail",
        vec![a_cell(), a_cell()],
        vec![],
        30,
    )
    .await
    .unwrap();
    let got = dashboard_get(&store, &test, ws, "meters").await.unwrap();
    assert_eq!(got.cells.len(), 2);
    assert_eq!(got.export_profiles, authored);
}

/// The other half of the same contract, in its own test so a failure names which direction broke:
/// an EMPTY array CLEARS. This is the delete-my-last-profile path.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn export_profiles_clear_on_empty() {
    let ws = "ws-profiles-clear";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws);

    dashboard_save_meta(
        &store,
        &test,
        ws,
        "meters",
        "Meter Detail",
        PageMeta {
            export_profiles: Some(vec![profile(
                "only",
                "The only one",
                ExportOptions::default(),
            )]),
            ..PageMeta::default()
        },
        vec![a_cell()],
        vec![],
        10,
    )
    .await
    .unwrap();
    assert_eq!(
        dashboard_get(&store, &test, ws, "meters")
            .await
            .unwrap()
            .export_profiles
            .len(),
        1
    );

    dashboard_save_meta(
        &store,
        &test,
        ws,
        "meters",
        "Meter Detail",
        PageMeta {
            export_profiles: Some(vec![]),
            ..PageMeta::default()
        },
        vec![a_cell()],
        vec![],
        20,
    )
    .await
    .unwrap();
    assert!(
        dashboard_get(&store, &test, ws, "meters")
            .await
            .unwrap()
            .export_profiles
            .is_empty(),
        "an explicit [] must clear — it is the only way to delete the last profile"
    );
}

/// The WIRE shape, over the real tool boundary — the claim the whole typed field exists to make.
///
/// A profile authored as camelCase JSON comes back out of `dashboard.get` as camelCase JSON, options
/// and all. Before the field was typed this is precisely what failed: `dashboard.save` accepted the
/// key, answered 200, and the record it wrote had already dropped it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_profile_survives_the_tool_boundary_with_its_option_vocabulary() {
    let ws = "ws-profiles-wire";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws);

    call_dashboard_tool(
        &store,
        &test,
        ws,
        "dashboard.save",
        &json!({
            "id": "energy", "title": "Energy", "now": 10,
            "cells": [{ "i": "c1", "x": 0, "y": 0, "w": 12, "h": 6, "v": 3, "view": "stat" }],
            "exportProfiles": [{
                "id": "a3-landscape",
                "name": "A3 landscape",
                "options": {
                    "paper": "a3",
                    "orientation": "landscape",
                    "marginXMm": 12.0,
                    "pageNumbers": true,
                    "index": true
                }
            }]
        }),
    )
    .await
    .expect("save");

    let got = call_dashboard_tool(
        &store,
        &test,
        ws,
        "dashboard.get",
        &json!({ "id": "energy" }),
    )
    .await
    .expect("get");

    let profiles = got["exportProfiles"].as_array().expect("exportProfiles");
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0]["id"], "a3-landscape");
    assert_eq!(profiles[0]["name"], "A3 landscape");
    // The option vocabulary is `report.export`'s own, unchanged — one spelling, not two.
    assert_eq!(profiles[0]["options"]["paper"], "a3");
    assert_eq!(profiles[0]["options"]["orientation"], "landscape");
    assert_eq!(profiles[0]["options"]["marginXMm"], 12.0);
    assert_eq!(profiles[0]["options"]["pageNumbers"], true);
    assert_eq!(profiles[0]["options"]["index"], true);

    // A board with no profiles keeps the key OFF the wire, so a pre-profiles record round-trips
    // byte-clean rather than growing an empty array.
    call_dashboard_tool(
        &store,
        &test,
        ws,
        "dashboard.save",
        &json!({
            "id": "plain", "title": "Plain", "now": 20,
            "cells": [{ "i": "c1", "x": 0, "y": 0, "w": 12, "h": 6, "v": 3, "view": "stat" }]
        }),
    )
    .await
    .expect("save");
    let plain = call_dashboard_tool(
        &store,
        &test,
        ws,
        "dashboard.get",
        &json!({ "id": "plain" }),
    )
    .await
    .expect("get");
    assert!(plain.get("exportProfiles").is_none());
}
