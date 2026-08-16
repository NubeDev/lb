//! Host-authored ext nav boards — WHAT IS A LEGAL RECORD (host-authored-ext-nav-boards scope).
//! The round-trip/deny/isolation half is `nav_ext_boards_test.rs`; the dispatcher and pin halves are
//! `nav_ext_boards_gate_test.rs`.
//!
//! Every bound and shape rule in one place, and every one of them asserted as `BadInput` — never a
//! silent truncation and never a silently-dropped row. The slot-grammar check is the one worth
//! naming: a key outside `ext:<id>` / `ext:<id>/<navid>` would bind rows to a slot no renderer looks
//! at, which is silent data loss dressed as a successful save.

//!   anticipated (that shape comes from an unknown-verb dispatch, and `nav.` is a known family, so
//!   the arm is reached and only the cap question is wrong). A deny-path assertion therefore CANNOT
//!   distinguish "correctly denied" from "alias missing" on this codebase: both are `Denied`. The
//!   real tripwire is the POSITIVE test — an admin holding only the canonical nav caps reaching
//!   both verbs. Both are below; deleting either alias fails
//!   `an_admin_reaches_both_verbs_over_the_dispatcher` (and the read half of the deny test).

use std::collections::BTreeMap;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    ext_nav_boards_get, ext_nav_boards_set, NavError, NavExtBoardRow, NAV_MAX_EXT_BOARD_ROWS,
    NAV_MAX_EXT_BOARD_SLOTS,
};
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

// ── Bounds + shape — rejected loudly, never truncated ──────────────────────────────────────────

/// Every bound is `BadInput`, never a silent truncation or a silently-dropped row: an over-cap
/// slot map, an over-cap slot, a blank/mis-grammared slot ref, an empty row id, a `/` in a row id
/// (it is ONE ref segment), a missing dashboard ref, and a duplicated row id within a slot.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn bounds_and_shape_violations_are_rejected() {
    let ws = "ws-eb-bounds";
    let store = Store::memory().await.unwrap();
    let admin = principal("user:ada", ws, &[SAVE, RESOLVE]);
    let bad = |m: BTreeMap<String, Vec<NavExtBoardRow>>| async {
        let e = ext_nav_boards_set(&store, &admin, ws, m, 1)
            .await
            .unwrap_err();
        assert!(
            matches!(e, NavError::BadInput(_)),
            "expected BadInput, got {e:?}"
        );
    };

    let too_many_slots: BTreeMap<_, _> = (0..=NAV_MAX_EXT_BOARD_SLOTS)
        .map(|i| (format!("ext:e{i}"), vec![row("a", "dashboard:b")]))
        .collect();
    bad(too_many_slots).await;

    let too_many_rows: Vec<_> = (0..=NAV_MAX_EXT_BOARD_ROWS)
        .map(|i| row(&format!("r{i}"), "dashboard:b"))
        .collect();
    bad(slots(&[("ext:alpha", too_many_rows)])).await;

    bad(slots(&[("   ", vec![row("a", "dashboard:b")])])).await;
    // Not the slot GRAMMAR (`ext:<id>` / `ext:<id>/<navid>`) — a key outside it would bind rows to a
    // slot no renderer looks at: silent data loss dressed as a successful save.
    bad(slots(&[("dashboard:x", vec![row("a", "dashboard:b")])])).await;
    bad(slots(&[("ext:", vec![row("a", "dashboard:b")])])).await;

    bad(slots(&[("ext:alpha", vec![row("", "dashboard:b")])])).await;
    bad(slots(&[("ext:alpha", vec![row("a/b", "dashboard:b")])])).await;
    // A `:` could forge or break the `b:` ref marker that tells a host row from a DECLARED
    // destination (`ext_boards_pin`), so a row id stays a plain slug.
    bad(slots(&[("ext:alpha", vec![row("a:b", "dashboard:b")])])).await;
    bad(slots(&[("ext:alpha", vec![row("a", "  ")])])).await;
    bad(slots(&[(
        "ext:alpha",
        vec![row("dup", "dashboard:b"), row("dup", "dashboard:c")],
    )]))
    .await;

    // Nothing from any rejected write reached the store.
    assert!(ext_nav_boards_get(&store, &admin, ws)
        .await
        .unwrap()
        .slots
        .is_empty());
}
