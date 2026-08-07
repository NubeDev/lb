//! The dashboard `kind` field (reports-as-dashboards scope) — headless, real store (`mem://`), real
//! write path. A report IS a dashboard whose record says so, so `kind` has to round-trip, ride the
//! CHEAP roster row (which is the whole reason it is a typed field rather than an `options` key),
//! survive a plain layout save, read as `"dashboard"` when absent, and refuse an unknown value
//! loudly rather than storing a typo that hides the record from both rosters.
//!
//! Its own file rather than another test in `dashboard_test.rs`: that file is already 700+ lines and
//! FILE-LAYOUT's ratchet only lets the backlog shrink.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    dashboard_get, dashboard_list, dashboard_save, dashboard_save_meta, Cell, CellSource,
    CellTarget, DashboardError, PageMeta,
};
use lb_store::Store;
use serde_json::json;

/// A principal `sub` in workspace `ws` holding `caps`.
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
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

const ALL: &[&str] = &[
    "mcp:dashboard.get:call",
    "mcp:dashboard.list:call",
    "mcp:dashboard.save:call",
];

/// A minimal v2 chart cell over `series`.
fn chart_cell(series: &str) -> Cell {
    Cell {
        i: series.into(),
        x: 0,
        y: 0,
        w: 6,
        h: 4,
        v: 2,
        view: "timeseries".into(),
        source: CellSource {
            tool: "series.read".into(),
            args: json!({ "series": series }),
        },
        sources: vec![CellTarget {
            ref_id: "A".into(),
            tool: "series.read".into(),
            args: json!({ "series": series }),
            ..CellTarget::default()
        }],
        ..Cell::default()
    }
}

// reports-as-dashboards scope: the typed `kind` field. A report IS a dashboard whose record says so,
// so `kind` must (a) round-trip through save/get, (b) ride the CHEAP roster summary — that is the whole
// reason it is a typed field and not an `options` key, since the roster is where the two kinds are
// partitioned — (c) survive a plain layout save (preserve-on-omit, or the first drag turns a report back
// into a dashboard), (d) read as "dashboard" when absent so every pre-kind record needs no migration,
// and (e) refuse an unknown value LOUDLY rather than storing a typo that hides the record from both lists.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn dashboard_kind_round_trips_preserves_and_validates() {
    let ws = "ws-dash-kind";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws, ALL);

    // (d) A board created with NO kind is a dashboard — empty, not "dashboard", and not a report.
    dashboard_save(
        &store,
        &test,
        ws,
        "ops",
        "Ops",
        vec![chart_cell("cooler.temp")],
        vec![],
        10,
    )
    .await
    .unwrap();
    let plain = dashboard_get(&store, &test, ws, "ops").await.unwrap();
    assert_eq!(plain.kind, "");
    assert!(!plain.is_report());

    // (a) Create a REPORT-kind record.
    dashboard_save_meta(
        &store,
        &test,
        ws,
        "energy",
        "Monthly Energy Report",
        PageMeta {
            kind: Some("report".into()),
            ..PageMeta::default()
        },
        vec![chart_cell("meter.kwh")],
        vec![],
        20,
    )
    .await
    .unwrap();
    let got = dashboard_get(&store, &test, ws, "energy").await.unwrap();
    assert_eq!(got.kind, "report");
    assert!(got.is_report());

    // (b) The cheap roster row carries the kind, so a list call partitions both surfaces.
    let roster = dashboard_list(&store, &test, ws).await.unwrap();
    let report_row = roster.iter().find(|s| s.id == "energy").unwrap();
    let dash_row = roster.iter().find(|s| s.id == "ops").unwrap();
    assert_eq!(report_row.kind, "report");
    assert_eq!(dash_row.kind, "");
    assert_eq!(
        roster.iter().filter(|s| s.kind == "report").count(),
        1,
        "exactly one report in the roster"
    );

    // (c) A plain layout save sends no kind — the report must still be a report afterwards.
    dashboard_save(
        &store,
        &test,
        ws,
        "energy",
        "Monthly Energy Report",
        vec![chart_cell("meter.kwh"), chart_cell("meter.kva")],
        vec![],
        30,
    )
    .await
    .unwrap();
    let got = dashboard_get(&store, &test, ws, "energy").await.unwrap();
    assert_eq!(got.cells.len(), 2);
    assert!(got.is_report(), "a layout save must not demote a report");

    // ...and so does a partial meta save that touches a different field.
    dashboard_save_meta(
        &store,
        &test,
        ws,
        "energy",
        "Monthly Energy Report",
        PageMeta {
            icon: Some("file-text".into()),
            ..PageMeta::default()
        },
        got.cells.clone(),
        vec![],
        40,
    )
    .await
    .unwrap();
    let got = dashboard_get(&store, &test, ws, "energy").await.unwrap();
    assert_eq!(got.icon, "file-text");
    assert!(got.is_report(), "preserve-on-omit holds for kind");

    // An EXPLICIT demotion is still possible — preserve-on-omit is not a one-way door.
    dashboard_save_meta(
        &store,
        &test,
        ws,
        "energy",
        "Monthly Energy Report",
        PageMeta {
            kind: Some("dashboard".into()),
            ..PageMeta::default()
        },
        got.cells.clone(),
        vec![],
        50,
    )
    .await
    .unwrap();
    let got = dashboard_get(&store, &test, ws, "energy").await.unwrap();
    assert_eq!(got.kind, "dashboard");
    assert!(!got.is_report());

    // (e) An unknown kind is refused loudly. Stored, it would drop the record out of BOTH rosters —
    // a "successful" save whose result cannot be found anywhere.
    let err = dashboard_save_meta(
        &store,
        &test,
        ws,
        "typo",
        "Typo",
        PageMeta {
            kind: Some("reprot".into()),
            ..PageMeta::default()
        },
        vec![],
        vec![],
        60,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, DashboardError::BadInput(ref m) if m.contains("reprot")),
        "unknown kind must be a loud BadInput, got {err:?}"
    );
    // ...and nothing was written.
    assert!(dashboard_get(&store, &test, ws, "typo").await.is_err());
}

/// The **bound reports** page-setting (`reportIds`) — what the Generate-report control offers.
///
/// Typed for the same reason `kind` is: `Dashboard` drops unknown top-level keys, so an untyped
/// `reportIds` would vanish on the first save. It carries the same preserve-on-omit discipline, with
/// one deliberate asymmetry: an EMPTY array is an explicit CLEAR, because an admin has to be able to
/// unbind every report and "omit to clear" would make that impossible.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn bound_report_ids_round_trip_preserve_on_omit_and_clear_on_empty() {
    let ws = "ws-dash-bound";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws, ALL);

    let save = |ids: Option<Vec<String>>, now: u64| {
        let store = &store;
        let test = &test;
        async move {
            dashboard_save_meta(
                store,
                test,
                ws,
                "meters",
                "Meter Detail",
                PageMeta {
                    report_ids: ids,
                    ..PageMeta::default()
                },
                vec![chart_cell("meter.kwh")],
                vec![],
                now,
            )
            .await
            .unwrap()
        }
    };

    // Absent on create ⇒ empty (no control), not a missing-field error.
    save(None, 10).await;
    assert!(dashboard_get(&store, &test, ws, "meters")
        .await
        .unwrap()
        .report_ids
        .is_empty());

    // Bind two.
    save(Some(vec!["energy".to_string(), "demand".to_string()]), 20).await;
    let got = dashboard_get(&store, &test, ws, "meters").await.unwrap();
    assert_eq!(got.report_ids, vec!["energy", "demand"]);

    // A plain LAYOUT save sends no binding — the binding must survive, or the first panel drag
    // silently unbinds every report an admin attached to the page.
    dashboard_save(
        &store,
        &test,
        ws,
        "meters",
        "Meter Detail",
        vec![chart_cell("meter.kwh"), chart_cell("meter.kva")],
        vec![],
        30,
    )
    .await
    .unwrap();
    let got = dashboard_get(&store, &test, ws, "meters").await.unwrap();
    assert_eq!(got.cells.len(), 2);
    assert_eq!(got.report_ids, vec!["energy", "demand"]);

    // An EMPTY array clears — the unbind path.
    save(Some(vec![]), 40).await;
    assert!(dashboard_get(&store, &test, ws, "meters")
        .await
        .unwrap()
        .report_ids
        .is_empty());
}
