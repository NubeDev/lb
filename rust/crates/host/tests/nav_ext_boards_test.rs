//! Host-authored ext nav boards (`nav.ext_boards.*`) — the host-owned, persisted binding of a host
//! dashboard into an extension's sidebar section, authored WITHOUT the extension's cooperation
//! (host-authored-ext-nav-boards scope, "Testing plan").
//!
//! Real store, real `Node`, real dispatcher — no mocks (rule 9). Two layers are exercised on
//! purpose, because they fail differently:
//!
//! - the **verb** (`ext_nav_boards_get`/`_set` called directly): bounds, LWW, workspace isolation;
//! - the **dispatcher** (`call_tool` → `gate_tool_for` → `call_nav_tool`): the CAP-ALIAS. A verb
//!   riding an existing cap is gated on its own namesake by default, and no `nav.ext_boards.*` cap
//!   exists in any bundle — so without the alias BOTH verbs refuse EVERY caller, admins included,
//!   while every direct-call test stays green (they never cross this gate).
//!
//!   **What the refusal looks like here, measured by deleting the alias and re-running:** the outer
//!   gate answers a bare `ToolError::Denied` — NOT the `NotFound`/"no such tool" the scope
//!   anticipated (that shape comes from an unknown-verb dispatch, and `nav.` is a known family, so
//!   the arm is reached and only the cap question is wrong). A deny-path assertion therefore CANNOT
//!   distinguish "correctly denied" from "alias missing" on this codebase: both are `Denied`. The
//!   real tripwire is the POSITIVE test — an admin holding only the canonical nav caps reaching
//!   both verbs. Both are below; deleting either alias fails
//!   `an_admin_reaches_both_verbs_over_the_dispatcher` (and the read half of the deny test).

use std::collections::BTreeMap;
use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    call_tool, ext_nav_boards_get, ext_nav_boards_set, NavError, NavExtBoardRow, Node,
    NAV_MAX_EXT_BOARD_ROWS, NAV_MAX_EXT_BOARD_SLOTS,
};
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

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

/// The deny path over the real dispatcher: a caller without `nav.save` gets a real `Denied` —
/// asserted by SHAPE, so a `NotFound` (an unknown verb: a missing dispatch arm or a typo'd name)
/// fails loudly instead of reading as a correct refusal. The second half is the load-bearing one:
/// that same member's READ dispatches, which only holds while the read alias points at
/// `nav.resolve`. Without it the rows an admin placed would be invisible to every member.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_deny_over_the_dispatcher_is_a_real_denial_not_no_such_tool() {
    let ws = "ws-eb-alias";
    let node = Arc::new(Node::boot().await.unwrap());
    let member = principal("user:ben", ws, &[RESOLVE]);

    let err = call(
        &node,
        &member,
        ws,
        "nav.ext_boards.set",
        json!({ "slots": { "ext:alpha": [] }, "now": 1 }),
    )
    .await
    .expect_err("a member without nav.save is refused");
    assert!(
        matches!(err, ToolError::Denied),
        "expected a REAL denial; a NotFound here means the gate_tool_for alias is missing: {err:?}"
    );

    // And the READ dispatches for that same member — proving the read alias lands on `nav.resolve`.
    call(&node, &member, ws, "nav.ext_boards.get", json!({}))
        .await
        .expect("the read is member-level (alias → nav.resolve)");
}

/// **THE GATE-ALIAS TRIPWIRE.** An admin holding ONLY the canonical `mcp:nav.save:call` +
/// `mcp:nav.resolve:call` — and NO `mcp:nav.ext_boards.set:call`, which exists in no bundle —
/// reaches both verbs over the dispatcher and reads its own write back.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_admin_reaches_both_verbs_over_the_dispatcher() {
    let ws = "ws-eb-dispatch";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = principal("user:ada", ws, &[SAVE, RESOLVE]);

    let written = call(
        &node,
        &admin,
        ws,
        "nav.ext_boards.set",
        json!({
            "slots": { "ext:alpha/sites": [
                { "id": "iaq", "dashboard": "dashboard:board-iaq", "label": "Indoor Air Quality",
                  "vars": { "site": "site-1" } }
            ] },
            "now": 5
        }),
    )
    .await
    .expect("nav.ext_boards.set dispatches (alias → nav.save)");
    assert_eq!(
        written["updated_ts"], 5,
        "the write echoes the record: {written}"
    );

    let got = call(&node, &admin, ws, "nav.ext_boards.get", json!({}))
        .await
        .expect("nav.ext_boards.get dispatches (alias → nav.resolve)");
    let rows = got["slots"]["ext:alpha/sites"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["dashboard"], "dashboard:board-iaq");
    assert_eq!(rows[0]["vars"]["site"], "site-1", "vars survive the bridge");
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

// ── Pinnable (Decision 2) — the property that makes a host row structurally better ─────────────

/// **Decision 2, proven end to end.** A host row is pinnable BECAUSE its ref is stable without a
/// mount — and that only holds if `nav.resolve` can resolve the ref. Both slot grammars would
/// otherwise strip silently (a section-root row reads as a declared destination the manifest does
/// not have; an item row has two slashes, which the shipped subref split deliberately refuses as a
/// runtime published child). A pinned host row must resolve to a DASHBOARD entry carrying its vars,
/// and round-trip back to the exact ref the shell pinned.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_pinned_host_row_resolves_to_its_board_in_both_slot_grammars() {
    let ws = "ws-eb-pin";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = principal(
        "user:ada",
        ws,
        &[
            SAVE,
            RESOLVE,
            "mcp:dashboard.save:call",
            "mcp:dashboard.get:call",
            "mcp:ext.list:call",
        ],
    );
    lb_host::dashboard_save(
        &node.store,
        &admin,
        ws,
        "board-iaq",
        "IAQ",
        vec![],
        vec![],
        1,
    )
    .await
    .expect("seed the board");

    let mut root_row = row("iaq", "dashboard:board-iaq");
    root_row.vars = [("site".to_string(), "site-1".to_string())]
        .into_iter()
        .collect();
    ext_nav_boards_set(
        &node.store,
        &admin,
        ws,
        slots(&[
            ("ext:alpha", vec![root_row]),
            (
                "ext:alpha/sites",
                vec![row("nested", "dashboard:board-iaq")],
            ),
        ]),
        1,
    )
    .await
    .unwrap();

    lb_host::nav_pref_set(
        &node.store,
        &admin,
        ws,
        None,
        Some(vec![
            "ext:alpha/iaq".into(),
            "ext:alpha/sites/nested".into(),
        ]),
        2,
    )
    .await
    .unwrap();

    let r = lb_host::nav_resolve(&node, &admin, ws).await.unwrap();
    assert_eq!(
        r.pinned.len(),
        2,
        "both host-row pins resolve: {:?}",
        r.pinned
    );

    // The section-root row: opens the board, var-bound, and still identified as its ext destination
    // so the pin lights the right rail row.
    assert_eq!(r.pinned[0].kind, "dashboard");
    assert_eq!(r.pinned[0].dashboard, "dashboard:board-iaq");
    assert_eq!(
        r.pinned[0].vars.get("site").map(String::as_str),
        Some("site-1")
    );
    assert_eq!(r.pinned[0].ext, "alpha");
    assert_eq!(r.pinned[0].nav, "iaq");

    // The row under a declared item: the SAME resolution through the two-segment `nav`, so the ref
    // reconstructs as `ext:alpha/sites/nested`.
    assert_eq!(r.pinned[1].kind, "dashboard");
    assert_eq!(r.pinned[1].ext, "alpha");
    assert_eq!(r.pinned[1].nav, "sites/nested");
}

/// A pin whose row was REMOVED from the record strips silently (the shipped invariant: a strip never
/// faults the menu and never mutates the stored pin, so re-adding the row restores it for free). And
/// hide beats pin here as everywhere.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_pin_for_a_removed_row_strips_without_faulting() {
    let ws = "ws-eb-pin-strip";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = principal(
        "user:ada",
        ws,
        &[SAVE, RESOLVE, "mcp:dashboard.get:call", "mcp:ext.list:call"],
    );
    lb_host::nav_pref_set(
        &node.store,
        &admin,
        ws,
        None,
        Some(vec!["ext:alpha/gone".into()]),
        1,
    )
    .await
    .unwrap();

    let r = lb_host::nav_resolve(&node, &admin, ws).await.unwrap();
    assert!(
        r.pinned.is_empty(),
        "a pin naming no row strips: {:?}",
        r.pinned
    );
    // The stored record is untouched — a strip is silent, never destructive.
    let pref = lb_host::nav_pref_get(&node.store, &admin, ws)
        .await
        .unwrap();
    assert_eq!(pref.pinned, vec!["ext:alpha/gone".to_string()]);
}
