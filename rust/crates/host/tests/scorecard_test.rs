//! `rule.scorecard` — the detector feedback loop, over a REAL booted `Node`
//! (`docs/scope/insights/case-plane-scope.md` §7, "Feedback from resolution to detection").
//!
//! Real store, real bus, real caps, the real `call_tool` MCP bridge. NO mocks (CLAUDE §9): every
//! insight is raised through `insight.raise`, every case is the one the grouping reactor opened for
//! it, and every outcome is written by `case.workflow`. The scorecard is then read back through the
//! verb, which is the only path a UI has.
//!
//! Mandatory categories: **capability-deny**, **workspace-isolation**, and the **POSITIVE gate
//! test** — a principal holding ONLY `mcp:rule.scorecard:call` must actually reach it, because a
//! cap that exists in no bundle (or a missing `tool_gate.rs` arm) is `Denied`, not `NotFound`, and
//! that refusal is indistinguishable from a real authorization failure
//! (`new-lb-verb-needs-a-gate-alias.md`). This trap has bitten this branch twice.
//!
//! The arithmetic itself has unit tests in `lb_cases::scorecard`; what is proved HERE is the join:
//! that `origin.ref` and `case.resolution` meet correctly after a real round trip through the
//! `{ data, rev }` store envelope.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_authz::membership_add_raw;
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

const RAISE: &str = "mcp:insight.raise:call";
const I_GET: &str = "mcp:insight.get:call";
const GET: &str = "mcp:case.get:call";
const WORKFLOW: &str = "mcp:case.workflow:call";
const SCORECARD: &str = "mcp:rule.scorecard:call";

/// The fully-empowered operator: raise findings, close cases, read the scorecard.
const ALL: &[&str] = &[RAISE, I_GET, GET, WORKFLOW, SCORECARD];

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

async fn seed_roster(node: &Arc<Node>, ws: &str) {
    membership_add_raw(&node.store, ws, "user:test", 1)
        .await
        .expect("test joins");
}

/// Raise one insight under `origin_ref`, optionally at `site`, and return its id. Grouping runs
/// inline at raise, so this also mints the `single` case the scorecard will later count.
async fn seed_insight(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    key: &str,
    origin_ref: &str,
    site: Option<&str>,
    ts: u64,
) -> String {
    let mut input = json!({
        "dedup_key": key,
        "severity": "warning",
        "title": format!("finding {key}"),
        "origin": { "kind": "rule", "ref": origin_ref },
        "ts": ts,
    });
    if let Some(site) = site {
        input["tags"] = json!({ "site": site });
    }
    let out = call(node, p, ws, "insight.raise", input)
        .await
        .expect("raise ok");
    out["id"].as_str().unwrap().to_string()
}

/// The case the grouping reactor opened for `insight_id`, read through the same `case_id` echo a
/// roster renders from.
async fn case_of(node: &Arc<Node>, p: &Principal, ws: &str, insight_id: &str) -> String {
    let out = call(node, p, ws, "insight.get", json!({ "id": insight_id }))
        .await
        .expect("get ok");
    out["case_id"]
        .as_str()
        .unwrap_or_else(|| panic!("insight {insight_id} has no case_id echo: {out}"))
        .to_string()
}

/// Raise a finding and close its case with `resolution` at `resolved_ts` — one whole outcome, the
/// unit the scorecard counts. Returns the insight id.
#[allow(clippy::too_many_arguments)]
async fn outcome(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    key: &str,
    origin_ref: &str,
    site: Option<&str>,
    opened_ts: u64,
    resolution: &str,
    resolved_ts: u64,
) -> String {
    let insight = seed_insight(node, p, ws, key, origin_ref, site, opened_ts).await;
    let case_id = case_of(node, p, ws, &insight).await;
    call(
        node,
        p,
        ws,
        "case.workflow",
        json!({
            "id": case_id,
            "workflow": "resolved",
            "resolution": resolution,
            "ts": resolved_ts,
        }),
    )
    .await
    .expect("case resolves");
    insight
}

async fn scorecard(node: &Arc<Node>, p: &Principal, ws: &str, args: Value) -> Vec<Value> {
    let out = call(node, p, ws, "rule.scorecard", args)
        .await
        .expect("scorecard ok");
    out["rows"].as_array().cloned().unwrap_or_default()
}

/// The one row for `(rule_ref, site)` in a scorecard result.
fn row<'a>(rows: &'a [Value], rule_ref: &str, site: Option<&str>) -> &'a Value {
    rows.iter()
        .find(|r| r["rule_ref"] == json!(rule_ref) && r.get("site").and_then(Value::as_str) == site)
        .unwrap_or_else(|| panic!("no row for ({rule_ref}, {site:?}) in {rows:#?}"))
}

// --- MANDATORY: capability deny -----------------------------------------------------------------

/// A principal without `mcp:rule.scorecard:call` is refused — including one that holds every OTHER
/// case cap. The scorecard is a read, but it is still a gated read.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_scorecard_denies_a_principal_without_its_cap() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let full = principal("user:test", "nube", ALL);
    outcome(
        &node,
        &full,
        "nube",
        "deny-probe",
        "rule:probe",
        None,
        1,
        "fixed",
        2,
    )
    .await;

    let bare = principal("key:nightly-rule", "nube", &[RAISE, I_GET, GET, WORKFLOW]);
    let err = call(&node, &bare, "nube", "rule.scorecard", json!({}))
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::Denied),
        "rule.scorecard must be Denied without its capability, got {err:?}"
    );
}

// --- MANDATORY: workspace isolation -------------------------------------------------------------

/// A ws-B principal never sees ws-A's outcomes. The cross-workspace call is refused outright, and a
/// legitimate call in B — same rule id, same site names — reports only B's own cases.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_b_principal_never_sees_ws_a_outcomes() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    seed_roster(&node, "other").await;

    let a = principal("user:test", "nube", ALL);
    outcome(
        &node,
        &a,
        "nube",
        "iso-1",
        "rule:shared",
        Some("north"),
        1,
        "fixed",
        2,
    )
    .await;
    outcome(
        &node,
        &a,
        "nube",
        "iso-2",
        "rule:shared",
        Some("north"),
        1,
        "fixed",
        2,
    )
    .await;

    let b = principal("user:test", "other", ALL);
    outcome(
        &node,
        &b,
        "other",
        "iso-3",
        "rule:shared",
        Some("north"),
        1,
        "false_positive",
        2,
    )
    .await;

    // The outer gate refuses the cross-workspace call before anything is read.
    let err = call(&node, &b, "nube", "rule.scorecard", json!({}))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "got {err:?}");

    // And B's own scorecard carries only B's single false positive — not A's two fixes.
    let rows = scorecard(&node, &b, "other", json!({})).await;
    let r = row(&rows, "rule:shared", Some("north"));
    assert_eq!(r["raised"], json!(1));
    assert_eq!(r["false_positive"], json!(1));
    assert_eq!(r["fixed"], json!(0));
    assert_eq!(r["precision"], json!(0.0));

    // A's own scorecard is untouched by B.
    let rows_a = scorecard(&node, &a, "nube", json!({})).await;
    let ra = row(&rows_a, "rule:shared", Some("north"));
    assert_eq!(ra["raised"], json!(2));
    assert_eq!(ra["precision"], json!(1.0));
}

// --- MANDATORY: the POSITIVE gate test ----------------------------------------------------------

/// The test a cap-in-no-bundle (or a missing `tool_gate.rs` arm) fails and nothing else does.
///
/// `rule.scorecard` gates on its OWN name, so it needs no alias — but it DOES need
/// `mcp:rule.scorecard:call` to live in the viewer bundle and the verb to be dispatched by exact
/// name in `tool_call.rs`. Miss either and every caller, admins included, gets a bare `Denied` that
/// looks exactly like a real authorization failure. Only a call that MUST succeed catches it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_principal_holding_only_the_scorecard_cap_can_actually_call_it() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let full = principal("user:test", "nube", ALL);
    outcome(
        &node,
        &full,
        "nube",
        "gate-1",
        "rule:gate",
        None,
        1,
        "fixed",
        5,
    )
    .await;

    // Nothing but the scorecard cap — no case read, no case write, no insight cap.
    let reader = principal("user:viewer", "nube", &[SCORECARD]);
    let out = call(&node, &reader, "nube", "rule.scorecard", json!({}))
        .await
        .expect("rule.scorecard must be reachable holding only its own cap");
    let rows = out["rows"].as_array().cloned().unwrap_or_default();
    assert_eq!(row(&rows, "rule:gate", None)["fixed"], json!(1));
}

/// And the cap is in the SHIPPED viewer bundle, not merely in a hand-written token — a verb only a
/// bespoke cap list can reach is shipped-but-unusable.
#[test]
fn the_scorecard_cap_ships_in_the_viewer_bundle() {
    let viewer = lb_host::viewer_role_caps();
    assert!(
        viewer.iter().any(|c| c == SCORECARD),
        "mcp:rule.scorecard:call must be in VIEWER_CAPS; viewer holds {viewer:?}"
    );
}

// --- The join: origin.ref × resolution -----------------------------------------------------------

/// The scope's formula, end to end through real records: two fixes against one false positive and
/// one self-cleared case ⇒ 0.5, and `accepted_risk`/`duplicate` are counted without moving it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn precision_is_fixed_over_fixed_plus_false_positive_plus_self_cleared() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let site = Some("north");

    for (key, resolution) in [
        ("f1", "fixed"),
        ("f2", "fixed"),
        ("fp", "false_positive"),
        ("sc", "self_cleared"),
        ("ar", "accepted_risk"),
        ("dup", "duplicate"),
    ] {
        outcome(
            &node,
            &p,
            "nube",
            key,
            "rule:alpha",
            site,
            1,
            resolution,
            10,
        )
        .await;
    }

    let rows = scorecard(&node, &p, "nube", json!({})).await;
    let r = row(&rows, "rule:alpha", site);
    assert_eq!(r["raised"], json!(6));
    assert_eq!(r["fixed"], json!(2));
    assert_eq!(r["false_positive"], json!(1));
    assert_eq!(r["self_cleared"], json!(1));
    assert_eq!(r["accepted_risk"], json!(1));
    assert_eq!(r["duplicate"], json!(1));
    // 2 / (2 + 1 + 1) — the accepted risk and the duplicate are reported but out of the denominator.
    assert_eq!(r["precision"], json!(0.5));
}

/// A rule whose only resolved cases are `accepted_risk`/`duplicate` has a ZERO denominator, and the
/// verb says so by OMITTING `precision` — never `0.0`, which a UI would render as "always wrong"
/// and defame a detector that has simply never been judged.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_zero_denominator_renders_as_no_opinion_never_zero() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    outcome(
        &node,
        &p,
        "nube",
        "ar",
        "rule:beta",
        None,
        1,
        "accepted_risk",
        5,
    )
    .await;
    outcome(
        &node,
        &p,
        "nube",
        "dup",
        "rule:beta",
        None,
        1,
        "duplicate",
        5,
    )
    .await;

    let rows = scorecard(&node, &p, "nube", json!({})).await;
    let r = row(&rows, "rule:beta", None);
    assert_eq!(r["raised"], json!(2));
    assert!(
        r.get("precision").is_none(),
        "a zero denominator must omit `precision`, got {r:#?}"
    );
    assert_ne!(r["precision"], json!(0.0));
}

/// The median time-to-resolve rides the real `opened_ts`/`resolved_ts` a case carries, and only the
/// `fixed` cases vote: a 1 ms false positive beside three fixes of 100/200/300 ms must not drag the
/// "how long does a fix take" answer below 200.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_median_is_over_the_fixed_cases_only() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    for (key, opened, resolved) in [
        ("m1", 1_000, 1_100),
        ("m2", 1_000, 1_300),
        ("m3", 1_000, 1_200),
    ] {
        outcome(
            &node,
            &p,
            "nube",
            key,
            "rule:gamma",
            None,
            opened,
            "fixed",
            resolved,
        )
        .await;
    }
    outcome(
        &node,
        &p,
        "nube",
        "mfp",
        "rule:gamma",
        None,
        1_000,
        "false_positive",
        1_001,
    )
    .await;

    let rows = scorecard(&node, &p, "nube", json!({})).await;
    // 100/200/300 — the odd-count median is the middle value; the 1 ms false positive did not vote.
    assert_eq!(
        row(&rows, "rule:gamma", None)["median_resolve_ms"],
        json!(200)
    );

    // A fourth fix makes it even: (200 + 300) / 2, the MEAN of the two middle values.
    outcome(
        &node,
        &p,
        "nube",
        "m4",
        "rule:gamma",
        None,
        1_000,
        "fixed",
        1_500,
    )
    .await;
    let rows = scorecard(&node, &p, "nube", json!({})).await;
    assert_eq!(
        row(&rows, "rule:gamma", None)["median_resolve_ms"],
        json!(250)
    );
}

/// Grouping is by the PAIR, and the site-less group survives: one rule at two sites plus one case
/// with no site is three rows, and the no-site row is neither folded into a sited one nor dropped.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn grouping_is_by_rule_ref_and_site_and_the_no_site_group_survives() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    outcome(
        &node,
        &p,
        "nube",
        "n1",
        "rule:delta",
        Some("north"),
        1,
        "fixed",
        5,
    )
    .await;
    outcome(
        &node,
        &p,
        "nube",
        "s1",
        "rule:delta",
        Some("south"),
        1,
        "false_positive",
        5,
    )
    .await;
    outcome(
        &node,
        &p,
        "nube",
        "x1",
        "rule:delta",
        None,
        1,
        "self_cleared",
        5,
    )
    .await;

    let rows = scorecard(&node, &p, "nube", json!({})).await;
    assert_eq!(rows.len(), 3, "three groups, not two: {rows:#?}");
    assert_eq!(
        row(&rows, "rule:delta", Some("north"))["precision"],
        json!(1.0)
    );
    assert_eq!(
        row(&rows, "rule:delta", Some("south"))["precision"],
        json!(0.0)
    );
    let none_row = row(&rows, "rule:delta", None);
    assert_eq!(none_row["self_cleared"], json!(1));
    assert_eq!(none_row["precision"], json!(0.0));
}

/// A resolved case whose primary insight has since been DELETED is counted under `unknown`, not
/// silently dropped. The work happened; the detector's name did not survive. Dropping it would
/// shrink the denominator and make every surviving precision a lie about a smaller population than
/// the reader believes they are looking at.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_deleted_primary_insight_is_counted_under_unknown_not_dropped() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    outcome(&node, &p, "nube", "keep", "rule:eps", None, 1, "fixed", 5).await;
    let doomed = outcome(
        &node,
        &p,
        "nube",
        "gone",
        "rule:eps",
        None,
        1,
        "false_positive",
        5,
    )
    .await;

    // The record disappears; its CASE, and the outcome somebody recorded on it, do not.
    lb_insights::delete(&node.store, "nube", &doomed)
        .await
        .expect("insight deleted");

    let rows = scorecard(&node, &p, "nube", json!({})).await;
    // The named rule keeps only the outcome it can still be credited with.
    let named = row(&rows, "rule:eps", None);
    assert_eq!(named["raised"], json!(1));
    assert_eq!(named["fixed"], json!(1));
    assert_eq!(named["precision"], json!(1.0));
    // And the orphan is visible rather than vanished.
    let unknown = row(&rows, lb_host::UNKNOWN_RULE_REF, None);
    assert_eq!(unknown["raised"], json!(1));
    assert_eq!(unknown["false_positive"], json!(1));
    assert_eq!(unknown["precision"], json!(0.0));
}

// --- Filters -------------------------------------------------------------------------------------

/// `rule_ref`, `site` and the `since`/`until` window each narrow the read, and an OPEN case is
/// never counted — a scorecard is a fold over finished work.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_filters_narrow_and_an_unresolved_case_is_never_counted() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    outcome(
        &node,
        &p,
        "nube",
        "old",
        "rule:zeta",
        Some("north"),
        1_000,
        "fixed",
        2_000,
    )
    .await;
    outcome(
        &node,
        &p,
        "nube",
        "new",
        "rule:zeta",
        Some("north"),
        1_000,
        "false_positive",
        9_000,
    )
    .await;
    outcome(
        &node,
        &p,
        "nube",
        "other-site",
        "rule:zeta",
        Some("south"),
        1_000,
        "fixed",
        2_000,
    )
    .await;
    outcome(
        &node,
        &p,
        "nube",
        "other-rule",
        "rule:eta",
        Some("north"),
        1_000,
        "fixed",
        2_000,
    )
    .await;
    // Raised and never resolved — it has no outcome, so it contributes nothing.
    seed_insight(
        &node,
        &p,
        "nube",
        "still-open",
        "rule:zeta",
        Some("north"),
        1_000,
    )
    .await;

    let all = scorecard(&node, &p, "nube", json!({})).await;
    assert_eq!(row(&all, "rule:zeta", Some("north"))["raised"], json!(2));

    let by_rule = scorecard(&node, &p, "nube", json!({ "rule_ref": "rule:zeta" })).await;
    assert!(
        by_rule.iter().all(|r| r["rule_ref"] == json!("rule:zeta")),
        "the rule_ref filter leaked: {by_rule:#?}"
    );

    let by_site = scorecard(&node, &p, "nube", json!({ "site": "south" })).await;
    assert_eq!(by_site.len(), 1);
    assert_eq!(by_site[0]["site"], json!("south"));

    // The window is on `resolved_ts` and inclusive at both ends: only the case closed at 9_000.
    let windowed = scorecard(
        &node,
        &p,
        "nube",
        json!({ "since": 3_000, "until": 10_000 }),
    )
    .await;
    assert_eq!(windowed.len(), 1, "{windowed:#?}");
    assert_eq!(windowed[0]["rule_ref"], json!("rule:zeta"));
    assert_eq!(windowed[0]["false_positive"], json!(1));
    assert_eq!(windowed[0]["fixed"], json!(0));
}
