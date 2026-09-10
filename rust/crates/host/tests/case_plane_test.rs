//! The **case plane** — a case is a piece of work that cites insights
//! (`docs/scope/insights/case-plane-scope.md`), over a REAL booted `Node`. Real store, real bus,
//! real caps, the real `call_tool` MCP bridge. NO mocks (CLAUDE §9): every record is created by
//! calling the verb under test and read back through it.
//!
//! Mandatory categories, per verb: **capability-deny** (a principal without the cap → `Denied`) and
//! **workspace-isolation** (a ws-B principal cannot see or touch a ws-A case).
//!
//! Plus the scope's named cases: the resolution invariant, the exclusivity invariant, workflow
//! immunity under 50 re-raises, the `Mine` lane resolving through TEAMS, and — the one that catches
//! the mistake this table exists to prevent — a POSITIVE gate test per aliased verb, because a
//! missing `tool_gate.rs` arm is `Denied`, not `NotFound`.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_authz::{membership_add_raw, team_create, MEMBER};
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

const RAISE: &str = "mcp:insight.raise:call";
const I_GET: &str = "mcp:insight.get:call";
const I_ASSIGN: &str = "mcp:insight.assign:call";
const I_COMMENT: &str = "mcp:insight.comment:call";
const GET: &str = "mcp:case.get:call";
const LIST: &str = "mcp:case.list:call";
const OPEN: &str = "mcp:case.open:call";
const WORKFLOW: &str = "mcp:case.workflow:call";

/// Every case cap plus the insight caps a test needs to seed with — the "fully-empowered operator".
const ALL: &[&str] = &[RAISE, I_GET, I_ASSIGN, I_COMMENT, GET, LIST, OPEN, WORKFLOW];

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

fn raise_input(dedup_key: &str, ts: u64) -> Value {
    json!({
        "dedup_key": dedup_key,
        "severity": "warning",
        "title": format!("finding {dedup_key}"),
        "origin": { "kind": "rule", "ref": "rule:probe" },
        "ts": ts,
    })
}

/// Raise one insight and return its id. Grouping runs inline, so this also mints its case.
async fn seed_insight(node: &Arc<Node>, p: &Principal, ws: &str, key: &str, ts: u64) -> String {
    let out = call(node, p, ws, "insight.raise", raise_input(key, ts))
        .await
        .expect("raise ok");
    out["id"].as_str().unwrap().to_string()
}

/// The case the grouping reactor opened for `insight_id` — read back through `insight.get`'s echo,
/// which is the same path a roster uses.
async fn case_of(node: &Arc<Node>, p: &Principal, ws: &str, insight_id: &str) -> String {
    let out = call(node, p, ws, "insight.get", json!({ "id": insight_id }))
        .await
        .expect("get ok");
    out["case_id"]
        .as_str()
        .unwrap_or_else(|| panic!("insight {insight_id} has no case_id echo: {out}"))
        .to_string()
}

/// A real workspace roster: `user:test` and `user:priya` are members, `team:mechanical` exists with
/// priya on it. Real rows through the real writers — no fixtures (CLAUDE §9).
async fn seed_roster(node: &Arc<Node>, ws: &str) {
    membership_add_raw(&node.store, ws, "user:test", 1)
        .await
        .expect("test joins");
    membership_add_raw(&node.store, ws, "user:priya", 1)
        .await
        .expect("priya joins");
    team_create(&node.store, ws, "team:mechanical", "Mechanical crew")
        .await
        .expect("team created");
    lb_assets::relate(&node.store, ws, MEMBER, "team:mechanical", "user:priya")
        .await
        .expect("priya joins the crew");
}

// --- MANDATORY: capability deny -----------------------------------------------------------------

/// Every case verb refuses a principal that does not hold its capability. One test over the whole
/// surface, because the interesting property is that there is NO hole: a verb added later without a
/// cap check fails here rather than shipping open.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn every_case_verb_denies_a_principal_without_its_cap() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let full = principal("user:test", "nube", ALL);
    let insight = seed_insight(&node, &full, "nube", "deny-probe", 1).await;
    let case_id = case_of(&node, &full, "nube", &insight).await;

    // Holds the insight caps and nothing on the case plane — the producer's exact shape.
    let bare = principal("key:nightly-rule", "nube", &[RAISE, I_GET]);

    for (tool, input) in [
        ("case.get", json!({ "id": case_id })),
        ("case.list", json!({ "lane": "mine" })),
        ("case.members", json!({ "case_id": case_id })),
        ("case.events", json!({ "case_id": case_id })),
        (
            "case.open",
            json!({ "title": "t", "primary_insight": insight }),
        ),
        ("case.merge", json!({ "from": case_id, "into": case_id })),
        (
            "case.split",
            json!({ "from": case_id, "insight_ids": [insight], "title": "t" }),
        ),
        (
            "case.workflow",
            json!({ "id": case_id, "workflow": "actioned" }),
        ),
        (
            "case.assign",
            json!({ "id": case_id, "assignee": "user:priya" }),
        ),
        (
            "case.snooze",
            json!({ "id": case_id, "until": 9_999_999, "reason": "r" }),
        ),
        ("case.comment", json!({ "id": case_id, "text": "hi" })),
    ] {
        let err = call(&node, &bare, "nube", tool, input).await.unwrap_err();
        assert!(
            matches!(err, ToolError::Denied),
            "{tool} must be Denied without its capability, got {err:?}"
        );
    }
}

// --- MANDATORY: the POSITIVE gate test (the aliased verbs) --------------------------------------

/// The test a missing `tool_gate.rs` alias fails and nothing else does.
///
/// `case.members`/`case.events` gate on `case.get`; `case.merge`/`case.split` on `case.open`;
/// `case.assign`/`case.snooze`/`case.comment` on `case.workflow`. If any alias is missing, the outer
/// gate demands `mcp:case.<verb>:call` — a capability in NO role bundle — and the verb answers a
/// bare `Denied` for every caller including admins. That refusal is indistinguishable from a real
/// authorization failure, so only a call that MUST succeed can catch it
/// (`new-lb-verb-needs-a-gate-alias.md`).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_aliased_case_verb_resolves_to_a_cap_a_role_actually_holds() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let full = principal("user:test", "nube", ALL);
    let a = seed_insight(&node, &full, "nube", "alias-a", 1).await;
    let b = seed_insight(&node, &full, "nube", "alias-b", 2).await;
    let case_a = case_of(&node, &full, "nube", &a).await;
    let case_b = case_of(&node, &full, "nube", &b).await;

    // A token holding ONLY the read cap reaches both aliased reads.
    let reader = principal("user:test", "nube", &[GET]);
    call(
        &node,
        &reader,
        "nube",
        "case.members",
        json!({ "case_id": case_a }),
    )
    .await
    .expect("case.members must resolve through the `case.get` alias");
    call(
        &node,
        &reader,
        "nube",
        "case.events",
        json!({ "case_id": case_a }),
    )
    .await
    .expect("case.events must resolve through the `case.get` alias");

    // A token holding ONLY the triage cap reaches all three triage writes.
    let triager = principal("user:test", "nube", &[WORKFLOW]);
    call(
        &node,
        &triager,
        "nube",
        "case.comment",
        json!({ "id": case_a, "text": "note" }),
    )
    .await
    .expect("case.comment must resolve through the `case.workflow` alias");
    call(
        &node,
        &triager,
        "nube",
        "case.snooze",
        json!({ "id": case_a, "until": 9_999_999_999_999u64, "reason": "next quarter" }),
    )
    .await
    .expect("case.snooze must resolve through the `case.workflow` alias");
    call(
        &node,
        &triager,
        "nube",
        "case.assign",
        json!({ "id": case_a, "assignee": "user:priya" }),
    )
    .await
    .expect("case.assign must resolve through the `case.workflow` alias");

    // A token holding ONLY the grouping cap reaches merge (and, through it, split).
    let grouper = principal("user:test", "nube", &[OPEN]);
    call(
        &node,
        &grouper,
        "nube",
        "case.merge",
        json!({ "from": case_b, "into": case_a }),
    )
    .await
    .expect("case.merge must resolve through the `case.open` alias");
    call(
        &node,
        &grouper,
        "nube",
        "case.split",
        json!({ "from": case_a, "insight_ids": [b], "title": "split out" }),
    )
    .await
    .expect("case.split must resolve through the `case.open` alias");
}

/// The caps the aliases point at must EXIST in a shipped role bundle — the other half of the trap.
/// An alias onto a cap nobody carries is just as unreachable as no alias at all.
#[test]
fn the_case_caps_ship_in_a_role_bundle() {
    let viewer = lb_host::viewer_role_caps();
    let member = lb_host::member_role_caps();
    for cap in [GET, LIST] {
        assert!(
            viewer.iter().any(|c| c == cap),
            "{cap} must be a VIEWER cap"
        );
    }
    for cap in [OPEN, WORKFLOW] {
        assert!(
            member.iter().any(|c| c == cap),
            "{cap} must be a MEMBER cap"
        );
        assert!(
            !viewer.iter().any(|c| c == cap),
            "{cap} must NOT be a viewer cap — a bare viewer reads the queue, a member moves it"
        );
    }
}

// --- MANDATORY: workspace isolation -------------------------------------------------------------

/// A ws-B principal cannot see or touch a ws-A case — and the refusal for a REAL ws-A case id is
/// identical to the refusal for a fictional one, so a probe learns nothing about what exists.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_b_principal_cannot_see_or_touch_a_ws_a_case() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "acme").await;
    let a_user = principal("user:test", "acme", ALL);
    let insight = seed_insight(&node, &a_user, "acme", "iso-probe", 1).await;
    let case_id = case_of(&node, &a_user, "acme", &insight).await;

    // Fully-capped, but in another workspace.
    let b_user = principal("user:test", "other", ALL);

    // The outer gate refuses the cross-workspace call before anything is read.
    let err = call(&node, &b_user, "acme", "case.get", json!({ "id": case_id }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "cross-ws get: {err:?}");

    // And inside its OWN workspace, a ws-A case id simply is not there — the same answer a
    // fictional id gives, because the store read is namespace-scoped.
    let real = call(
        &node,
        &b_user,
        "other",
        "case.get",
        json!({ "id": case_id }),
    )
    .await
    .expect("in-ws get is allowed");
    let fake = call(
        &node,
        &b_user,
        "other",
        "case.get",
        json!({ "id": "01ABSENT" }),
    )
    .await
    .expect("in-ws get is allowed");
    assert_eq!(real, Value::Null);
    assert_eq!(
        real, fake,
        "a real ws-A id and a fictional id must be indistinguishable from ws-B"
    );

    // A write is refused the same way, and nothing lands.
    let err = call(
        &node,
        &b_user,
        "other",
        "case.workflow",
        json!({ "id": case_id, "workflow": "actioned" }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::BadInput(_)),
        "a ws-A case is simply absent in ws-B: {err:?}"
    );
    let still = call(&node, &a_user, "acme", "case.get", json!({ "id": case_id }))
        .await
        .expect("ws-A still reads its own case");
    assert_eq!(still["workflow"], "to_action", "ws-A's case was untouched");
}

// --- The resolution invariant -------------------------------------------------------------------

/// `case.workflow(resolved)` without a `resolution` is `BadInput`, and the case does not move.
///
/// This is the invariant that keeps a queue from becoming a graveyard: closed-with-no-reason cannot
/// be told from a false positive later, and the hold-down reactor has nothing to judge.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn resolved_without_a_resolution_is_bad_input_and_nothing_moves() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let insight = seed_insight(&node, &p, "nube", "res-probe", 1).await;
    let case_id = case_of(&node, &p, "nube", &insight).await;

    let err = call(
        &node,
        &p,
        "nube",
        "case.workflow",
        json!({ "id": case_id, "workflow": "resolved", "ts": 10 }),
    )
    .await
    .unwrap_err();
    match err {
        ToolError::BadInput(m) => assert!(
            m.contains("resolution"),
            "the refusal must name the missing field: {m}"
        ),
        other => panic!("expected BadInput, got {other:?}"),
    }

    let case = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert_eq!(case["workflow"], "to_action", "the case did not move");
    assert_eq!(case["closed"], false);

    // The mirror image: a resolution on a NON-terminal transition is refused too, so the two fields
    // can never disagree.
    let err = call(
        &node,
        &p,
        "nube",
        "case.workflow",
        json!({ "id": case_id, "workflow": "actioned", "resolution": "fixed", "ts": 11 }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::BadInput(_)), "{err:?}");

    // With a resolution, it closes — and stamps who and when.
    let closed = call(
        &node,
        &p,
        "nube",
        "case.workflow",
        json!({ "id": case_id, "workflow": "resolved", "resolution": "fixed", "ts": 12 }),
    )
    .await
    .expect("resolve ok");
    assert_eq!(closed["workflow"], "resolved");
    assert_eq!(closed["resolution"], "fixed");
    assert_eq!(closed["closed"], true);
    assert_eq!(closed["resolved_by"], "user:test");
    assert_eq!(closed["resolved_ts"], 12);
}

// --- The exclusivity invariant ------------------------------------------------------------------

/// **Every open insight is in exactly one open case.** Adding a detection already held by another
/// OPEN case is refused; `case.merge` is the verb that legitimately moves it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_open_insight_is_in_exactly_one_open_case() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let a = seed_insight(&node, &p, "nube", "excl-a", 1).await;
    let b = seed_insight(&node, &p, "nube", "excl-b", 2).await;
    let case_a = case_of(&node, &p, "nube", &a).await;
    let case_b = case_of(&node, &p, "nube", &b).await;
    assert_ne!(case_a, case_b, "two findings, two `single` cases");

    // Opening a THIRD case that cites `b` — already held by `case_b` — is refused.
    let err = call(
        &node,
        &p,
        "nube",
        "case.open",
        json!({ "title": "a second home for b", "primary_insight": b, "ts": 3 }),
    )
    .await
    .unwrap_err();
    match err {
        ToolError::BadInput(m) => assert!(
            m.contains("already a member of open case"),
            "the refusal must name the invariant: {m}"
        ),
        other => panic!("expected BadInput, got {other:?}"),
    }

    // Merge is the way. After it, `b` sits in `case_a` and `case_b` is closed as `duplicate`.
    call(
        &node,
        &p,
        "nube",
        "case.merge",
        json!({ "from": case_b, "into": case_a, "ts": 4 }),
    )
    .await
    .expect("merge ok");

    let members = call(
        &node,
        &p,
        "nube",
        "case.members",
        json!({ "case_id": case_a }),
    )
    .await
    .expect("members ok");
    let ids: Vec<&str> = members["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["insight_id"].as_str().unwrap())
        .collect();
    assert!(
        ids.contains(&a.as_str()) && ids.contains(&b.as_str()),
        "{ids:?}"
    );
    assert_eq!(members["total"], 2);

    let loser = call(&node, &p, "nube", "case.get", json!({ "id": case_b }))
        .await
        .expect("get ok");
    assert_eq!(loser["resolution"], "duplicate");
    assert_eq!(loser["closed"], true);

    // And the echo followed the member: `b` now points at `case_a`.
    assert_eq!(case_of(&node, &p, "nube", &b).await, case_a);
}

// --- Workflow immunity under re-raise -----------------------------------------------------------

/// **50 re-raises move nothing on the human plane.** A flapping sensor firing every 15 minutes must
/// never resurrect a case somebody parked, un-assign the technician who took the job, or reset a
/// workflow that is waiting on a purchase order. Only the member count and the detection's own
/// lifetime accounting may move.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn fifty_re_raises_leave_the_human_plane_untouched() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let insight = seed_insight(&node, &p, "nube", "flapper", 1).await;
    let case_id = case_of(&node, &p, "nube", &insight).await;

    call(
        &node,
        &p,
        "nube",
        "case.assign",
        json!({ "id": case_id, "assignee": "team:mechanical", "ts": 2 }),
    )
    .await
    .expect("assign ok");
    call(
        &node,
        &p,
        "nube",
        "case.workflow",
        json!({ "id": case_id, "workflow": "waiting_on_po", "waiting_on": "client", "ts": 3 }),
    )
    .await
    .expect("workflow ok");
    call(
        &node,
        &p,
        "nube",
        "case.snooze",
        json!({ "id": case_id, "until": 9_999_999_999_999u64, "reason": "PO with the client", "ts": 4 }),
    )
    .await
    .expect("snooze ok");

    let before = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    let members_before = call(
        &node,
        &p,
        "nube",
        "case.members",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("members ok");

    // The same dedup key, fifty more times, at the SAME severity (an escalation is the one thing
    // that is allowed to move a snooze, and it is tested separately).
    for i in 0..50u64 {
        call(
            &node,
            &p,
            "nube",
            "insight.raise",
            raise_input("flapper", 100 + i),
        )
        .await
        .expect("re-raise ok");
    }

    let after = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    for field in [
        "workflow",
        "waiting_on",
        "assigned_to",
        "snooze_until",
        "snooze_reason",
        "snoozed_by",
        "resolution",
        "impact_rate",
        "cost_to_fix",
        "verified_saving",
        "reopened_count",
        "id",
    ] {
        assert_eq!(
            before.get(field),
            after.get(field),
            "`{field}` moved under 50 re-raises"
        );
    }

    // And no second case was minted for the same detection.
    let members_after = call(
        &node,
        &p,
        "nube",
        "case.members",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("members ok");
    assert_eq!(members_before["total"], members_after["total"]);
    assert_eq!(case_of(&node, &p, "nube", &insight).await, case_id);
}

/// The ONE thing a re-raise may move: a severity **escalation punctures a snooze**. A case parked as
/// "look at it next quarter" that has since become critical is not still parked — leaving it hidden
/// is how a snooze turns into a way to lose a fault.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_severity_escalation_punctures_a_snooze() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let insight = seed_insight(&node, &p, "nube", "escalator", 1).await;
    let case_id = case_of(&node, &p, "nube", &insight).await;

    call(
        &node,
        &p,
        "nube",
        "case.snooze",
        json!({ "id": case_id, "until": 9_999_999_999_999u64, "reason": "next quarter", "ts": 2 }),
    )
    .await
    .expect("snooze ok");
    let parked = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert!(parked["snooze_until"].is_u64(), "parked: {parked}");

    // Same key, worse severity.
    let mut worse = raise_input("escalator", 200);
    worse["severity"] = json!("critical");
    call(&node, &p, "nube", "insight.raise", worse)
        .await
        .expect("re-raise ok");

    let after = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert!(
        after["snooze_until"].is_null(),
        "a severity escalation must puncture the snooze: {after}"
    );
    assert_eq!(after["severity"], "critical", "and carry the new severity");
    // The reason it came back is in the history, not only in the absence of a field.
    let events = call(
        &node,
        &p,
        "nube",
        "case.events",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("events ok");
    let punctured = events["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["kind"] == "snoozed" && e["data"]["punctured"] == true);
    assert!(punctured, "the puncture must be in the history: {events}");
}

// --- The Mine lane ------------------------------------------------------------------------------

/// The `Mine` lane is "the person plus every team they belong to" — the SAME definition
/// `SubFilter.assignee: "me"` already ships. A naive `assigned_to == principal.sub()` silently drops
/// every case assigned to a QUEUE the caller is on, which hides exactly the team-owned work the
/// `team:` subject decision exists to support.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_mine_lane_includes_the_teams_the_caller_is_on() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    let own = seed_insight(&node, &p, "nube", "mine-own", 1).await;
    let team = seed_insight(&node, &p, "nube", "mine-team", 2).await;
    let other = seed_insight(&node, &p, "nube", "mine-other", 3).await;
    let (own_case, team_case, other_case) = (
        case_of(&node, &p, "nube", &own).await,
        case_of(&node, &p, "nube", &team).await,
        case_of(&node, &p, "nube", &other).await,
    );

    call(
        &node,
        &p,
        "nube",
        "case.assign",
        json!({ "id": own_case, "assignee": "user:priya", "ts": 4 }),
    )
    .await
    .expect("assign ok");
    call(
        &node,
        &p,
        "nube",
        "case.assign",
        json!({ "id": team_case, "assignee": "team:mechanical", "ts": 5 }),
    )
    .await
    .expect("assign ok");
    call(
        &node,
        &p,
        "nube",
        "case.assign",
        json!({ "id": other_case, "assignee": "user:test", "ts": 6 }),
    )
    .await
    .expect("assign ok");

    // Priya is on `team:mechanical`, so her lane holds BOTH her own case and the crew's.
    let priya = principal("user:priya", "nube", ALL);
    let page = call(
        &node,
        &priya,
        "nube",
        "case.list",
        json!({ "lane": "mine" }),
    )
    .await
    .expect("list ok");
    let ids: Vec<&str> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert!(
        ids.contains(&own_case.as_str()),
        "her own case is missing: {ids:?}"
    );
    assert!(
        ids.contains(&team_case.as_str()),
        "the TEAM's case is missing — 'mine' must resolve through teams: {ids:?}"
    );
    assert!(
        !ids.contains(&other_case.as_str()),
        "somebody else's case leaked into her lane: {ids:?}"
    );

    // And the roster carries the member COUNT, never the members.
    for row in page["items"].as_array().unwrap() {
        assert!(row["member_count"].is_u64(), "no member_count on {row}");
        assert!(
            row.get("items").is_none() && row.get("members").is_none(),
            "case.list must never carry the members themselves: {row}"
        );
    }
}

/// The queue's order is the product decision: **`due_at` ascending, then severity descending**, with
/// un-clocked cases last. A `warning` due this afternoon outranks a `critical` due next month,
/// because the first is about to become a broken promise.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_lane_sorts_by_deadline_then_severity() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    // Three cases, deadlines written directly (the sla-clock reactor owns them in the product; the
    // ORDER is this verb's contract and is tested independently of who stamps the field).
    let mut ids = Vec::new();
    for (key, severity, due) in [
        ("sort-late-crit", "critical", Some(9_000u64)),
        ("sort-soon-warn", "warning", Some(1_000u64)),
        ("sort-never-crit", "critical", None),
    ] {
        let mut input = raise_input(key, 1);
        input["severity"] = json!(severity);
        let out = call(&node, &p, "nube", "insight.raise", input)
            .await
            .expect("raise ok");
        let case_id = case_of(&node, &p, "nube", out["id"].as_str().unwrap()).await;
        if let Some(due) = due {
            let mut case = lb_cases::get(&node.store, "nube", &case_id)
                .await
                .unwrap()
                .unwrap();
            case.due_at = Some(due);
            let value = serde_json::to_value(&case).unwrap();
            lb_store::write(&node.store, "nube", lb_cases::CASE_TABLE, &case_id, &value)
                .await
                .unwrap();
        }
        ids.push(case_id);
    }

    let page = call(
        &node,
        &p,
        "nube",
        "case.list",
        json!({ "lane": "watching" }),
    )
    .await
    .expect("list ok");
    let order: Vec<&str> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        order,
        vec![ids[1].as_str(), ids[0].as_str(), ids[2].as_str()],
        "expected soon-warning, then late-critical, then the un-clocked critical: {page}"
    );
}

/// The `party` filter is REFUSED rather than silently matching everything — the `case_request` plane
/// it needs is wave 2. A filter that quietly matches every row is how a queue lies.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_party_filter_refuses_rather_than_matching_everything() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let err = call(
        &node,
        &p,
        "nube",
        "case.list",
        json!({ "lane": "watching", "filter": { "party": "party:acme-mech" } }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::BadInput(_)), "{err:?}");
}

// --- Snooze + comment semantics -----------------------------------------------------------------

/// A snooze needs a REASON; un-snooze is `until: now` and needs none.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_snooze_needs_a_reason_and_un_snooze_is_until_now() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let insight = seed_insight(&node, &p, "nube", "snoozer", 1).await;
    let case_id = case_of(&node, &p, "nube", &insight).await;

    let err = call(
        &node,
        &p,
        "nube",
        "case.snooze",
        json!({ "id": case_id, "until": 9_999_999_999_999u64, "ts": 2 }),
    )
    .await
    .unwrap_err();
    match err {
        ToolError::BadInput(m) => assert!(m.contains("reason"), "{m}"),
        other => panic!("expected BadInput, got {other:?}"),
    }

    let parked = call(
        &node,
        &p,
        "nube",
        "case.snooze",
        json!({ "id": case_id, "until": 9_999_999_999_999u64, "reason": "tenant moves out in May", "ts": 3 }),
    )
    .await
    .expect("snooze ok");
    assert_eq!(parked["snooze_reason"], "tenant moves out in May");
    assert_eq!(parked["snoozed_by"], "user:test");

    // Un-snooze: `until` at or before now, no reason required.
    let live = call(
        &node,
        &p,
        "nube",
        "case.snooze",
        json!({ "id": case_id, "until": 10, "ts": 10 }),
    )
    .await
    .expect("un-snooze ok");
    assert!(live["snooze_until"].is_null(), "{live}");
    assert!(live["snooze_reason"].is_null(), "{live}");
}

/// The `snoozed` filter is evaluated against the caller's clock, so an EXPIRED snooze reads as live
/// without any sweep having run.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_expired_snooze_reads_as_live_without_a_sweep() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let insight = seed_insight(&node, &p, "nube", "expirer", 1).await;
    let case_id = case_of(&node, &p, "nube", &insight).await;
    call(
        &node,
        &p,
        "nube",
        "case.snooze",
        json!({ "id": case_id, "until": 5_000, "reason": "a week", "ts": 2 }),
    )
    .await
    .expect("snooze ok");

    let parked = call(
        &node,
        &p,
        "nube",
        "case.list",
        json!({ "lane": "watching", "now": 1_000, "filter": { "snoozed": true } }),
    )
    .await
    .expect("list ok");
    assert_eq!(parked["total"], 1, "parked at now=1000: {parked}");

    let expired = call(
        &node,
        &p,
        "nube",
        "case.list",
        json!({ "lane": "watching", "now": 9_000, "filter": { "snoozed": true } }),
    )
    .await
    .expect("list ok");
    assert_eq!(expired["total"], 0, "live again at now=9000: {expired}");
}

/// The history is append-only and carries every transition — the audit trail a client reads back.
/// `actor` is host-stamped from the principal, so a caller cannot forge another operator's note.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_history_records_every_transition_with_an_un_forgeable_actor() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let insight = seed_insight(&node, &p, "nube", "historian", 1).await;
    let case_id = case_of(&node, &p, "nube", &insight).await;

    call(
        &node,
        &p,
        "nube",
        "case.workflow",
        json!({ "id": case_id, "workflow": "actioned", "ts": 2 }),
    )
    .await
    .expect("ok");
    call(
        &node,
        &p,
        "nube",
        "case.assign",
        json!({ "id": case_id, "assignee": "user:priya", "ts": 3 }),
    )
    .await
    .expect("ok");
    // A forged author is IGNORED, not refused — the field is simply not read from the input.
    call(
        &node,
        &p,
        "nube",
        "case.comment",
        json!({ "id": case_id, "text": "replaced the sensor", "author": "user:someone-else", "ts": 4 }),
    )
    .await
    .expect("ok");

    let events = call(
        &node,
        &p,
        "nube",
        "case.events",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("events ok");
    let items = events["items"].as_array().unwrap();
    let kinds: Vec<&str> = items.iter().map(|e| e["kind"].as_str().unwrap()).collect();
    for expected in ["opened", "workflow", "assigned", "comment"] {
        assert!(kinds.contains(&expected), "missing `{expected}`: {kinds:?}");
    }
    // Newest first, and every seq unique.
    let seqs: Vec<u64> = items.iter().map(|e| e["eseq"].as_u64().unwrap()).collect();
    let mut sorted = seqs.clone();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(seqs, sorted, "history must be newest-first: {seqs:?}");

    let comment = items.iter().find(|e| e["kind"] == "comment").unwrap();
    assert_eq!(
        comment["actor"], "user:test",
        "the author is host-stamped, never taken from the input: {comment}"
    );
    // The reactor's own writes are visibly `system:`, not a person.
    let opened = items.iter().find(|e| e["kind"] == "opened").unwrap();
    assert_eq!(opened["actor"], "system:case-group");
}

// --- Split --------------------------------------------------------------------------------------

/// `case.split` is the human's escape hatch from a reactor's grouping, and what it produces is
/// permanent: the new case is `grouping: human` and its members are `human_placed`, so no reactor
/// may fold them back.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn split_produces_a_human_case_no_reactor_may_re_fold() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let a = seed_insight(&node, &p, "nube", "split-a", 1).await;
    let b = seed_insight(&node, &p, "nube", "split-b", 2).await;
    let case_a = case_of(&node, &p, "nube", &a).await;
    let case_b = case_of(&node, &p, "nube", &b).await;
    call(
        &node,
        &p,
        "nube",
        "case.merge",
        json!({ "from": case_b, "into": case_a, "ts": 3 }),
    )
    .await
    .expect("merge ok");

    // Splitting EVERY member out is refused — that gesture is a re-title, and it would leave a case
    // citing nothing.
    let err = call(
        &node,
        &p,
        "nube",
        "case.split",
        json!({ "from": case_a, "insight_ids": [a, b], "title": "all of it", "ts": 4 }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::BadInput(_)), "{err:?}");

    let new_case = call(
        &node,
        &p,
        "nube",
        "case.split",
        json!({ "from": case_a, "insight_ids": [b], "title": "a separate fault", "ts": 5 }),
    )
    .await
    .expect("split ok");
    assert_eq!(new_case["grouping"], "human");
    assert_eq!(new_case["primary_insight"], b);

    let members = call(
        &node,
        &p,
        "nube",
        "case.members",
        json!({ "case_id": new_case["id"] }),
    )
    .await
    .expect("members ok");
    assert!(
        members["items"][0]["human_placed"] == true,
        "a split member must be human_placed: {members}"
    );
    assert_eq!(case_of(&node, &p, "nube", &b).await, new_case["id"]);
}
