//! Host-authored ext nav boards — the DISPATCHER and the PIN (host-authored-ext-nav-boards scope).
//! The record's own round-trip, bounds and isolation are `nav_ext_boards_test.rs`.
//!
//! Two things that fail differently from the verb, and both only over the real bridge:
//!
//! - **the CAP-ALIAS.** A verb riding an existing cap is gated on its own namesake by default, and
//!   no `nav.ext_boards.*` cap exists in any bundle — so without the `gate_tool_for` aliases BOTH
//!   verbs refuse EVERY caller, admins included, while every direct-call test stays green (they
//!   never cross this gate).
//!
//!   **What the refusal looks like here, measured by deleting the alias and re-running:** the outer
//!   gate answers a bare `ToolError::Denied` — NOT the `NotFound`/"no such tool" the scope
//!   anticipated (that shape comes from an unknown-verb dispatch, and `nav.` is a known family, so
//!   the arm is reached and only the cap question is wrong). A deny-path assertion therefore CANNOT
//!   distinguish "correctly denied" from "alias missing" on this codebase: both are `Denied`. The
//!   real tripwire is the POSITIVE test — an admin holding only the canonical nav caps reaching
//!   both verbs. Deleting either alias fails `an_admin_reaches_both_verbs_over_the_dispatcher`.
//!
//! - **the PIN.** Decision 2 says host rows are pinnable, and that is the whole structural argument
//!   for them over published children. It only holds if `nav.resolve` can resolve the ref — and
//!   both ref shapes would otherwise strip SILENTLY.

//!   anticipated (that shape comes from an unknown-verb dispatch, and `nav.` is a known family, so
//!   the arm is reached and only the cap question is wrong). A deny-path assertion therefore CANNOT
//!   distinguish "correctly denied" from "alias missing" on this codebase: both are `Denied`. The
//!   real tripwire is the POSITIVE test — an admin holding only the canonical nav caps reaching
//!   both verbs. Both are below; deleting either alias fails
//!   `an_admin_reaches_both_verbs_over_the_dispatcher` (and the read half of the deny test).

use std::collections::BTreeMap;
use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, ext_nav_boards_set, NavExtBoardRow, Node};
use lb_mcp::ToolError;
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

// ── The cap alias, over the real dispatcher ───────────────────────────────────────────────────

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
