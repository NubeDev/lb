//! Host-authored ext nav boards — the RECORD and its verbs (host-authored-ext-nav-boards scope,
//! "Testing plan"). The bounds/shape half is `nav_ext_boards_bounds_test.rs`; the dispatcher
//! cap-alias and the pin half are `nav_ext_boards_gate_test.rs`. Split three ways because they
//! answer three different questions — and because one file was over the FILE-LAYOUT limit.
//!
//! Real store, real `Node` — no mocks (rule 9). Covered here: an absent record reads as the empty
//! map, full-set LWW at BOTH slot kinds with stored order preserved, a row's vars/icon round-tripping
//! verbatim, a dangling dashboard ref kept rather than dropped, capability-deny at the VERB (with
//! the read deliberately member-level, because every reached member's rail renders these rows), and
//! workspace isolation in both directions.
use std::collections::BTreeMap;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{ext_nav_boards_get, ext_nav_boards_set, NavError, NavExtBoardRow};
use lb_store::Store;

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

const SAVE: &str = "mcp:nav.save:call";
const RESOLVE: &str = "mcp:nav.resolve:call";

/// One row bound into a slot. The ids are opaque to the host — deliberately not any shipped
/// extension's (rule 10: nothing in this path may be recognisable to the core).
fn row(id: &str, dashboard: &str) -> NavExtBoardRow {
    NavExtBoardRow {
        id: id.into(),
        dashboard: dashboard.into(),
        label: format!("{id} board"),
        ..Default::default()
    }
}

fn slots(pairs: &[(&str, Vec<NavExtBoardRow>)]) -> BTreeMap<String, Vec<NavExtBoardRow>> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect()
}

// ── The record: absent == empty, full-set LWW ──────────────────────────────────────────────────

/// An absent record is the empty map — the feature is additive and inert until an admin uses it,
/// and a member on a node where nobody ever bound a board must not see an error.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn absent_record_reads_as_the_empty_map() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:ada", "ws-eb-absent", &[SAVE, RESOLVE]);
    let got = ext_nav_boards_get(&store, &admin, "ws-eb-absent")
        .await
        .unwrap();
    assert!(got.slots.is_empty(), "absent record is the empty map");
    assert_eq!(got.updated_ts, 0);
}

/// Round-trip both slot KINDS — the section root (`ext:<id>`) and a declared item
/// (`ext:<id>/<navid>`) — and prove the write is full-set LWW: the second save REPLACES the map.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn set_then_get_round_trips_both_slot_kinds_and_is_lww() {
    let ws = "ws-eb-rt";
    let store = Store::memory().await.unwrap();
    let admin = principal("user:ada", ws, &[SAVE, RESOLVE]);

    let written = ext_nav_boards_set(
        &store,
        &admin,
        ws,
        slots(&[
            ("ext:alpha", vec![row("iaq", "dashboard:board-iaq")]),
            (
                "ext:alpha/sites",
                vec![
                    row("energy", "dashboard:board-energy"),
                    row("water", "board-water"),
                ],
            ),
        ]),
        7,
    )
    .await
    .expect("admin writes");
    assert_eq!(written.updated_ts, 7);

    let got = ext_nav_boards_get(&store, &admin, ws).await.unwrap();
    assert_eq!(got.slots.len(), 2);
    assert_eq!(got.slots["ext:alpha"][0].dashboard, "dashboard:board-iaq");
    // Stored ORDER is render order — the shell appends these after the published children in
    // exactly this sequence, so the round-trip must not reorder them.
    let sites: Vec<&str> = got.slots["ext:alpha/sites"]
        .iter()
        .map(|r| r.id.as_str())
        .collect();
    assert_eq!(sites, vec!["energy", "water"], "stored order survives");

    // Full-set LWW: the second write replaces the whole map, it does not merge.
    ext_nav_boards_set(
        &store,
        &admin,
        ws,
        slots(&[("ext:alpha", vec![row("iaq", "dashboard:board-iaq")])]),
        9,
    )
    .await
    .unwrap();
    let got = ext_nav_boards_get(&store, &admin, ws).await.unwrap();
    assert_eq!(got.slots.len(), 1, "LWW replaces, never merges");
    assert!(!got.slots.contains_key("ext:alpha/sites"));

    // An empty map CLEARS the record (the tombstone shape `nav_hidden` uses).
    ext_nav_boards_set(&store, &admin, ws, BTreeMap::new(), 11)
        .await
        .unwrap();
    assert!(ext_nav_boards_get(&store, &admin, ws)
        .await
        .unwrap()
        .slots
        .is_empty());
}

/// A row's `vars` (the pinned `?var-` binding) and its icon fields round-trip verbatim — the shell
/// folds them into the viewer URL, so a dropped key is a silently wrong destination.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_rows_vars_and_icon_round_trip_verbatim() {
    let ws = "ws-eb-vars";
    let store = Store::memory().await.unwrap();
    let admin = principal("user:ada", ws, &[SAVE, RESOLVE]);
    let mut r = row("iaq", "dashboard:board-iaq");
    r.icon = "Gauge".into();
    r.icon_color = "#00aa55".into();
    r.vars = [("site".to_string(), "site-1".to_string())]
        .into_iter()
        .collect();

    ext_nav_boards_set(
        &store,
        &admin,
        ws,
        slots(&[("ext:alpha", vec![r.clone()])]),
        1,
    )
    .await
    .unwrap();
    let got = ext_nav_boards_get(&store, &admin, ws).await.unwrap();
    assert_eq!(got.slots["ext:alpha"][0], r, "the row round-trips whole");
}

/// A DANGLING dashboard ref (a board that was deleted, or never existed) is stored and returned
/// unchanged. The host never resolves the ref — dropping the row would hide a real config error
/// behind silence; the viewer's own not-found state is the honest answer (scope, "Testing plan").
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_dangling_dashboard_ref_is_stored_not_dropped() {
    let ws = "ws-eb-dangling";
    let store = Store::memory().await.unwrap();
    let admin = principal("user:ada", ws, &[SAVE, RESOLVE]);
    ext_nav_boards_set(
        &store,
        &admin,
        ws,
        slots(&[("ext:alpha", vec![row("ghost", "dashboard:no-such-board")])]),
        1,
    )
    .await
    .expect("a ref the host cannot resolve is still a legal ref");
    let got = ext_nav_boards_get(&store, &admin, ws).await.unwrap();
    assert_eq!(
        got.slots["ext:alpha"][0].dashboard,
        "dashboard:no-such-board"
    );
}

// ── Capability deny — the MANDATORY category, at both layers ───────────────────────────────────

/// Deny (verb layer): a member WITHOUT `mcp:nav.save:call` cannot write, and nothing persists. The
/// read is member-level on purpose — the same member CAN read, because every reached member's rail
/// renders these rows.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn set_is_denied_without_the_save_cap_but_the_read_is_member_level() {
    let ws = "ws-eb-deny";
    let store = Store::memory().await.unwrap();
    let admin = principal("user:ada", ws, &[SAVE, RESOLVE]);
    let member = principal("user:ben", ws, &[RESOLVE]);
    let nobody = principal("user:cal", ws, &[]);

    assert!(matches!(
        ext_nav_boards_set(
            &store,
            &member,
            ws,
            slots(&[("ext:alpha", vec![row("iaq", "dashboard:b")])]),
            1
        )
        .await
        .unwrap_err(),
        NavError::Denied
    ));
    // Nothing persisted.
    assert!(ext_nav_boards_get(&store, &admin, ws)
        .await
        .unwrap()
        .slots
        .is_empty());

    // The admin writes; the plain member READS it — that is the whole point of the split gate.
    ext_nav_boards_set(
        &store,
        &admin,
        ws,
        slots(&[("ext:alpha", vec![row("iaq", "dashboard:b")])]),
        2,
    )
    .await
    .unwrap();
    assert_eq!(
        ext_nav_boards_get(&store, &member, ws).await.unwrap().slots["ext:alpha"][0].id,
        "iaq",
        "a plain member sees the rows their rail must render"
    );

    // A caller with no nav caps at all reads nothing.
    assert!(matches!(
        ext_nav_boards_get(&store, &nobody, ws).await.unwrap_err(),
        NavError::Denied
    ));
}

// ── Workspace isolation — the MANDATORY category ───────────────────────────────────────────────

/// Rows written in ws A are invisible in ws B; the record never crosses the wall, in either
/// direction, even for the same subject holding the same caps.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_record_is_workspace_walled() {
    let store = Store::memory().await.unwrap();
    let in_a = principal("user:ada", "ws-eb-a", &[SAVE, RESOLVE]);
    let in_b = principal("user:ada", "ws-eb-b", &[SAVE, RESOLVE]);

    ext_nav_boards_set(
        &store,
        &in_a,
        "ws-eb-a",
        slots(&[("ext:alpha", vec![row("iaq", "dashboard:board-iaq")])]),
        1,
    )
    .await
    .unwrap();

    assert!(
        ext_nav_boards_get(&store, &in_b, "ws-eb-b")
            .await
            .unwrap()
            .slots
            .is_empty(),
        "ws-B sees nothing of ws-A's record"
    );
    // A ws-A token cannot reach into ws-B either (workspace-first gate).
    assert!(matches!(
        ext_nav_boards_get(&store, &in_a, "ws-eb-b")
            .await
            .unwrap_err(),
        NavError::Denied
    ));
    // ws-A still holds its own.
    assert_eq!(
        ext_nav_boards_get(&store, &in_a, "ws-eb-a")
            .await
            .unwrap()
            .slots
            .len(),
        1
    );
}
