//! Regression tests for the single-scan-page read-back bug OUTSIDE flows (issue #69; same class as
//! debugging/flows/single-scan-page-drops-rows-past-200.md). Every roster read — dashboard / panel /
//! report / nav / brand / render_templates, plus `rules.list` and the insight sub loader — used ONE
//! `lb_store::scan` page (hard-capped at 200 rows) and filtered in code, so once a workspace outgrew a
//! page the roster silently dropped every row past it. These prove the paginated drain
//! (`lb_store::scan_all`, the shared seam every roster read now goes through) returns rows past the
//! boundary:
//!   - the canonical drain itself (250 rows in a scratch table, id-ordered),
//!   - a strict-decode roster with a visibility filter (`dashboard.list` — fillers are tombstoned so
//!     they decode but `list` skips them before gate 3), and
//!   - a loose-decode / authz-filtered roster (`rules.list` — junk fillers are swallowed by decode).
//!
//! The remaining CRUD stores (panel/report/nav/brand/render_templates) share the IDENTICAL
//! `scan_all` + envelope-unwrap shape as dashboard (verified by reading each); dashboard stands in
//! for that whole class, and the canonical-drain test pins the one function they all call.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{dashboard_list, dashboard_save, rules_list};
use lb_store::{scan_all, write, Store};
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

/// `lb_store::scan_all` drains every page: 250 real rows in a scratch table come back, in id order,
/// including the one a single 200-row page would have dropped. This pins the canonical seam every
/// roster read shares.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn scan_all_drains_every_page_in_id_order() {
    let ws = "ws-scan-drain";
    let store = Store::memory().await.unwrap();
    let table = "scan_drain_probe";

    for i in 0..250u32 {
        write(
            &store,
            ws,
            table,
            &format!("row-{i:04}"),
            &json!({ "i": i }),
        )
        .await
        .unwrap();
    }

    let rows = scan_all(&store, ws, table).await.unwrap();
    assert_eq!(rows.len(), 250, "every row past the 200-row page returns");
    // id-ordered ascending — the cursor order the grid + every roster relies on.
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted, "rows come back in id order");
    // row-0249 sorts LAST — exactly the row a single-page read dropped.
    assert_eq!(rows.last().unwrap().id, "scan_drain_probe:row-0249");
}

/// `dashboard.list` returns the caller's own dashboard even when 240 OTHER dashboards sort before it
/// (past one 200-row page). The fillers are tombstoned valid records: they decode (the scan decodes
/// every row) but `list` skips them before the gate-3 visibility check — so this proves the
/// strict-decode roster drains AND the visibility filter still runs at scale.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn dashboard_list_returns_target_past_one_scan_page() {
    let ws = "ws-dash-page";
    let store = Store::memory().await.unwrap();
    let p = principal(
        "user:ada",
        ws,
        &["mcp:dashboard.save:call", "mcp:dashboard.list:call"],
    );

    // 240 tombstoned dashboards, ids `aaa-fill-XXXX` — they sort before `zzz-target`.
    for i in 0..240u32 {
        write(
            &store,
            ws,
            "dashboard",
            &format!("aaa-fill-{i:04}"),
            &json!({ "id": format!("aaa-fill-{i:04}"), "title": "fill", "owner": "nobody", "updated_ts": 0, "deleted": true }),
        )
        .await
        .unwrap();
    }

    dashboard_save(&store, &p, ws, "zzz-target", "Target", vec![], vec![], 1)
        .await
        .unwrap();

    let roster = dashboard_list(&store, &p, ws).await.unwrap();
    assert!(
        roster.iter().any(|s| s.id == "zzz-target"),
        "the owner's dashboard must appear past one scan page: {roster:?}"
    );
    assert!(
        !roster.iter().any(|s| s.id.starts_with("aaa-fill-")),
        "tombstoned fillers never appear in the roster: {roster:?}"
    );
}

/// `rules.list` returns a real rule even when 240 filler rows sort before it (past one 200-row page).
/// The fillers are junk that fails `SavedRule` decode — `rules_list` swallows them — so the one real
/// rule sorting last is the only thing that can come back. Proves the authz-filtered rules roster
/// drains past the boundary.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rules_list_returns_target_past_one_scan_page() {
    let ws = "ws-rules-page";
    let store = Store::memory().await.unwrap();
    let p = principal("user:ada", ws, &["store:rule:read"]);

    // 240 filler rows that fail SavedRule decode (junk) — they occupy pages but `rules_list` swallows
    // them, so the one real rule sorting last is the only thing that can come back.
    for i in 0..240u32 {
        write(
            &store,
            ws,
            "rule",
            &format!("aaa-fill-{i:04}"),
            &json!({ "junk": i }),
        )
        .await
        .unwrap();
    }

    write(
        &store,
        ws,
        "rule",
        "zzz-target",
        &json!({ "id": "zzz-target", "name": "Target", "body": "1" }),
    )
    .await
    .unwrap();

    let rules = rules_list(&store, &p, ws).await.unwrap();
    assert!(
        rules.iter().any(|r| r.id == "zzz-target"),
        "the real rule must appear past one scan page: {rules:?}"
    );
}
