//! The **case-group**, **hold-down** and **reconcile** reactors over a REAL booted `Node`
//! (`docs/scope/insights/case-plane-scope.md` §"Reactors"). Real store, real bus, real caps, the
//! real `call_tool` MCP bridge. NO mocks.
//!
//! **Every reactor case asserts RED first.** Grouping is an inline effect of `insight.raise`, so the
//! capability that gates it is `mcp:insight.raise:call` — the reactor principal's own grant. Each
//! test therefore runs the identical call with that cap REMOVED, asserts the refusal AND that no
//! case was created, then runs it with the cap and asserts the case appears. A reactor test that
//! only ever ran green proves nothing (`green-while-broken-reactor-tests.md`): without the RED half
//! a grouping function that returned a hard-coded id, or one that never ran at all, would pass.
//!
//! The two orderings a scheduled rule really produces — **verdict-first** (the citing record beats
//! the findings it cites) and **verdict-last** (the findings already sit in `single` cases) — each
//! get their own case, because they exercise completely different code paths and converge on the
//! same answer only if both are right.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_authz::membership_add_raw;
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

const RAISE: &str = "mcp:insight.raise:call";
const I_GET: &str = "mcp:insight.get:call";
const I_COMMENT: &str = "mcp:insight.comment:call";
const I_ASSIGN: &str = "mcp:insight.assign:call";
const GET: &str = "mcp:case.get:call";
const LIST: &str = "mcp:case.list:call";
const OPEN: &str = "mcp:case.open:call";
const WORKFLOW: &str = "mcp:case.workflow:call";

const ALL: &[&str] = &[RAISE, I_GET, I_COMMENT, I_ASSIGN, GET, LIST, OPEN, WORKFLOW];

/// The reactor principal's caps MINUS the raise grant — the RED half of every case below.
const NO_RAISE: &[&str] = &[I_GET, I_COMMENT, I_ASSIGN, GET, LIST, OPEN, WORKFLOW];

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

/// A VERDICT record, in the shape the real producer writes: `explains[]` holds the **dedup_key**s of
/// the findings it accounts for, and `root_cause` names the upstream fault it blames.
///
/// The other body keys the producer writes (`equip`, `root_issue`, `first_repair`, `note`) are
/// present deliberately: the grouping must read `root_cause` and `explains` and be completely
/// indifferent to the rest — that indifference IS rule 10, and a test that omitted them would not
/// notice a grouping that had started keying off `equip`.
fn verdict_input(dedup_key: &str, root_cause: &str, explains: &[&str], ts: u64) -> Value {
    json!({
        "dedup_key": dedup_key,
        "severity": "critical",
        "title": format!("verdict {dedup_key}"),
        "origin": { "kind": "rule", "ref": "rule:probe" },
        "ts": ts,
        "body": {
            "equip": "some-equip-ref",
            "root_cause": root_cause,
            "root_issue": "some issue text",
            "explains": explains,
            "first_repair": "some repair text",
            "note": "some note",
        },
    })
}

/// A raise straight through the CRATE — the shape a node running before the case plane existed left
/// behind, with no grouping and therefore no case. The backfill's whole reason for existing.
fn legacy_raise(
    dedup_key: &str,
    severity: lb_insights::Severity,
    title: &str,
) -> lb_insights::RaiseInput {
    lb_insights::RaiseInput {
        dedup_key: dedup_key.into(),
        severity,
        title: title.into(),
        body: Value::Null,
        evidence: None,
        analysis: None,
        origin: lb_insights::Origin::new(lb_insights::OriginKind::Rule, "rule:legacy", None),
        tags: Default::default(),
        occurrence: None,
        ts: 1_000,
        producer: "key:legacy".into(),
    }
}

async fn seed_roster(node: &Arc<Node>, ws: &str) {
    membership_add_raw(&node.store, ws, "user:test", 1)
        .await
        .expect("test joins");
    membership_add_raw(&node.store, ws, "user:priya", 1)
        .await
        .expect("priya joins");
}

/// The case the grouping opened for `insight_id`, via the `case_id` echo.
async fn case_of(node: &Arc<Node>, p: &Principal, ws: &str, insight_id: &str) -> String {
    let out = call(node, p, ws, "insight.get", json!({ "id": insight_id }))
        .await
        .expect("get ok");
    out["case_id"]
        .as_str()
        .unwrap_or_else(|| panic!("insight {insight_id} has no case_id echo: {out}"))
        .to_string()
}

/// How many open cases exist in the workspace — the number the invariant is really about.
async fn open_case_count(node: &Arc<Node>, p: &Principal, ws: &str) -> u64 {
    call(node, p, ws, "case.list", json!({ "lane": "watching" }))
        .await
        .expect("list ok")["total"]
        .as_u64()
        .unwrap()
}

/// The member ids of a case, sorted.
async fn member_ids(node: &Arc<Node>, p: &Principal, ws: &str, case_id: &str) -> Vec<String> {
    let page = call(node, p, ws, "case.members", json!({ "case_id": case_id }))
        .await
        .expect("members ok");
    let mut ids: Vec<String> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["insight_id"].as_str().unwrap().to_string())
        .collect();
    ids.sort();
    ids
}

// --- The invariant, RED then green --------------------------------------------------------------

/// **RED first.** With the raise cap removed the grouping never runs: the call is `Denied` and the
/// workspace has zero cases. Only then is the green half meaningful — a `single` case appears with
/// the finding as its primary.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn grouping_is_red_without_the_raise_cap_and_green_with_it() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let reader = principal("node:reactor", "nube", NO_RAISE);

    // RED — the reactor principal LACKS the cap that gates the path.
    let err = call(
        &node,
        &reader,
        "nube",
        "insight.raise",
        raise_input("g1", 1),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "expected Denied: {err:?}");
    assert_eq!(
        open_case_count(&node, &reader, "nube").await,
        0,
        "a denied raise must group nothing"
    );

    // GREEN — the same call, with the cap.
    let full = principal("node:reactor", "nube", ALL);
    let out = call(&node, &full, "nube", "insight.raise", raise_input("g1", 1))
        .await
        .expect("raise ok");
    let id = out["id"].as_str().unwrap();
    assert_eq!(open_case_count(&node, &full, "nube").await, 1);

    let case_id = case_of(&node, &full, "nube", id).await;
    let case = call(&node, &full, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert_eq!(case["grouping"], "single");
    assert_eq!(case["primary_insight"], id);
    assert_eq!(case["severity"], "warning", "the severity is echoed");
    assert_eq!(case["closed"], false);
    assert_eq!(member_ids(&node, &full, "nube", &case_id).await, vec![id]);
}

// --- Verdict: the two orderings -----------------------------------------------------------------

/// **verdict-LAST** — the cited findings already sit in their own `single` cases when the verdict
/// record arrives. The singles must be MERGED into one verdict case, not left as duplicates.
///
/// This is the ordering that catches a grouping which only ever adds: it would leave three open
/// cases citing four detections, the queue would over-count, and two technicians would work the same
/// fault from two rows.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn verdict_last_merges_the_existing_single_cases_into_one_verdict_case() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;

    // RED: without the raise cap nothing is grouped at all.
    let denied = principal("node:reactor", "nube", NO_RAISE);
    assert!(matches!(
        call(
            &node,
            &denied,
            "nube",
            "insight.raise",
            raise_input("flatline", 1)
        )
        .await
        .unwrap_err(),
        ToolError::Denied
    ));
    assert_eq!(open_case_count(&node, &denied, "nube").await, 0);

    // GREEN. Three ordinary findings first — three `single` cases.
    let p = principal("user:test", "nube", ALL);
    let root = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("flatline", 1),
    )
    .await
    .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let sym_a = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("valve-hunting", 2),
    )
    .await
    .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let sym_b = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("temp-drift", 3),
    )
    .await
    .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(open_case_count(&node, &p, "nube").await, 3, "three singles");

    // Now the verdict record — citing DEDUP KEYS, not ids.
    let verdict = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        verdict_input("verdict-1", "flatline", &["valve-hunting", "temp-drift"], 4),
    )
    .await
    .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();

    // ONE open case, and it is about the ROOT — not the record that named it.
    assert_eq!(
        open_case_count(&node, &p, "nube").await,
        1,
        "the singles must fold into the verdict case, not sit beside it"
    );
    let case_id = case_of(&node, &p, "nube", &root).await;
    let case = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert_eq!(case["grouping"], "verdict");
    assert_eq!(
        case["primary_insight"], root,
        "the primary is the ROOT finding, not the verdict record"
    );

    // All four detections — the root, both symptoms, and the verdict record itself — in one case.
    let mut expected = vec![root.clone(), sym_a.clone(), sym_b.clone(), verdict.clone()];
    expected.sort();
    assert_eq!(member_ids(&node, &p, "nube", &case_id).await, expected);

    // And every one of them carries the same `case_id` echo.
    for id in [&root, &sym_a, &sym_b, &verdict] {
        assert_eq!(
            case_of(&node, &p, "nube", id).await,
            case_id,
            "echo on {id}"
        );
    }
}

/// **verdict-FIRST** — the citing record arrives before the findings it cites exist. It opens a case
/// over what resolves (nothing but itself), and each straggler joins that case as it is raised.
///
/// Without the straggler lookup this ordering silently produces a permanently wrong grouping: the
/// verdict's `explains[]` resolved to nothing, so the stragglers get `single` cases and nobody ever
/// reconciles them. The bug is invisible — every record has a case, the invariant holds, the answer
/// is just wrong.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn verdict_first_folds_each_straggler_in_as_it_arrives() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;

    // RED.
    let denied = principal("node:reactor", "nube", NO_RAISE);
    assert!(matches!(
        call(
            &node,
            &denied,
            "nube",
            "insight.raise",
            verdict_input("verdict-2", "late-flatline", &["late-hunting"], 1)
        )
        .await
        .unwrap_err(),
        ToolError::Denied
    ));
    assert_eq!(open_case_count(&node, &denied, "nube").await, 0);

    // GREEN. The verdict record first: nothing it cites exists yet.
    let p = principal("user:test", "nube", ALL);
    let verdict = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        verdict_input("verdict-2", "late-flatline", &["late-hunting"], 1),
    )
    .await
    .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let case_id = case_of(&node, &p, "nube", &verdict).await;
    let case = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert_eq!(case["grouping"], "verdict");
    assert_eq!(
        case["primary_insight"], verdict,
        "with nothing resolvable, the citing record stands in as the primary"
    );
    assert_eq!(open_case_count(&node, &p, "nube").await, 1);

    // The root arrives. It must join the case that cited it, NOT open a `single`.
    let root = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("late-flatline", 2),
    )
    .await
    .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        open_case_count(&node, &p, "nube").await,
        1,
        "the root must join the citing case, not open a second one"
    );
    assert_eq!(case_of(&node, &p, "nube", &root).await, case_id);

    // Then the symptom.
    let sym = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("late-hunting", 3),
    )
    .await
    .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(open_case_count(&node, &p, "nube").await, 1);
    assert_eq!(case_of(&node, &p, "nube", &sym).await, case_id);

    let mut expected = vec![verdict, root, sym];
    expected.sort();
    assert_eq!(member_ids(&node, &p, "nube", &case_id).await, expected);
}

/// A member a PERSON placed is never moved by a reactor — not to fix a grouping, not to satisfy the
/// verdict. The grouping is left wrong rather than overruling a judgement the machine cannot see.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_reactor_never_moves_a_human_placed_member() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    let root = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("hp-root", 1),
    )
    .await
    .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let sym = call(&node, &p, "nube", "insight.raise", raise_input("hp-sym", 2))
        .await
        .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();

    // A person deliberately splits the symptom into its own case — `human_placed`.
    let root_case = case_of(&node, &p, "nube", &root).await;
    let sym_case = case_of(&node, &p, "nube", &sym).await;
    call(
        &node,
        &p,
        "nube",
        "case.merge",
        json!({ "from": sym_case, "into": root_case, "ts": 3 }),
    )
    .await
    .expect("merge ok");
    let human_case = call(
        &node,
        &p,
        "nube",
        "case.split",
        json!({ "from": root_case, "insight_ids": [sym], "title": "a separate fault", "ts": 4 }),
    )
    .await
    .expect("split ok")["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Now a verdict record claims the symptom. The reactor must NOT take it back.
    call(
        &node,
        &p,
        "nube",
        "insight.raise",
        verdict_input("hp-verdict", "hp-root", &["hp-sym"], 5),
    )
    .await
    .expect("ok");

    assert_eq!(
        case_of(&node, &p, "nube", &sym).await,
        human_case,
        "the human-placed member stayed where the person put it"
    );
    let human = call(&node, &p, "nube", "case.get", json!({ "id": human_case }))
        .await
        .expect("get ok");
    assert_eq!(human["closed"], false, "and its case is still open");
}

// --- Hold-down ----------------------------------------------------------------------------------

/// **`fixed` then a re-fire inside the window ⇒ the SAME case reopens**, `reopened_count: 1`, with a
/// `reopened` event saying the repair did not hold.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_re_fire_after_a_fix_reopens_the_same_case() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;

    // RED.
    let denied = principal("node:reactor", "nube", NO_RAISE);
    assert!(matches!(
        call(
            &node,
            &denied,
            "nube",
            "insight.raise",
            raise_input("hd-1", 1)
        )
        .await
        .unwrap_err(),
        ToolError::Denied
    ));
    assert_eq!(open_case_count(&node, &denied, "nube").await, 0);

    // GREEN.
    let p = principal("user:test", "nube", ALL);
    let day: u64 = 24 * 60 * 60 * 1000;
    let id = call(&node, &p, "nube", "insight.raise", raise_input("hd-1", day))
        .await
        .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let case_id = case_of(&node, &p, "nube", &id).await;

    call(
        &node,
        &p,
        "nube",
        "case.workflow",
        json!({ "id": case_id, "workflow": "resolved", "resolution": "fixed", "ts": 2 * day }),
    )
    .await
    .expect("resolve ok");
    assert_eq!(
        open_case_count(&node, &p, "nube").await,
        0,
        "the queue is clear"
    );

    // Three days later — well inside the default 14-day hold-down — the fault is back.
    call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("hd-1", 5 * day),
    )
    .await
    .expect("re-raise ok");

    assert_eq!(
        case_of(&node, &p, "nube", &id).await,
        case_id,
        "the SAME case must come back, not a new one"
    );
    let reopened = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert_eq!(reopened["workflow"], "to_action");
    assert_eq!(reopened["closed"], false);
    assert!(
        reopened["resolution"].is_null(),
        "a reopened case must not carry the last repair's closure reason: {reopened}"
    );
    assert!(reopened["resolved_ts"].is_null());
    assert_eq!(reopened["reopened_count"], 1);
    assert_eq!(open_case_count(&node, &p, "nube").await, 1);

    // The history says WHY it came back.
    let events = call(
        &node,
        &p,
        "nube",
        "case.events",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("events ok");
    let reopen_event = events["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "reopened")
        .unwrap_or_else(|| panic!("no `reopened` event: {events}"));
    assert_eq!(reopen_event["data"]["reason"], "repair did not hold");
}

/// **Any other resolution ⇒ a NEW case.** `false_positive` says the detection was wrong; a re-fire is
/// evidence about the rule, not about a repair that failed, so reopening would be a lie about what
/// happened.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_re_fire_after_a_false_positive_opens_a_new_case() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let day: u64 = 24 * 60 * 60 * 1000;

    let id = call(&node, &p, "nube", "insight.raise", raise_input("hd-2", day))
        .await
        .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let first = case_of(&node, &p, "nube", &id).await;
    call(
        &node,
        &p,
        "nube",
        "case.workflow",
        json!({ "id": first, "workflow": "resolved", "resolution": "false_positive", "ts": 2 * day }),
    )
    .await
    .expect("resolve ok");

    call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("hd-2", 3 * day),
    )
    .await
    .expect("re-raise ok");

    let second = case_of(&node, &p, "nube", &id).await;
    assert_ne!(
        second, first,
        "a false positive must not reopen — a NEW case"
    );
    let old = call(&node, &p, "nube", "case.get", json!({ "id": first }))
        .await
        .expect("get ok");
    assert_eq!(old["closed"], true, "and the old case stays closed");
    assert_eq!(old["resolution"], "false_positive");
}

/// **Outside the window ⇒ a new case**, even after a `fixed`. Nine months later the same fault is
/// next year's job, not last year's unfinished one.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_re_fire_long_after_a_fix_opens_a_new_case() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let day: u64 = 24 * 60 * 60 * 1000;

    let id = call(&node, &p, "nube", "insight.raise", raise_input("hd-3", day))
        .await
        .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let first = case_of(&node, &p, "nube", &id).await;
    call(
        &node,
        &p,
        "nube",
        "case.workflow",
        json!({ "id": first, "workflow": "resolved", "resolution": "fixed", "ts": 2 * day }),
    )
    .await
    .expect("resolve ok");

    // 300 days later — far outside any sane hold-down.
    call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("hd-3", 300 * day),
    )
    .await
    .expect("re-raise ok");

    assert_ne!(
        case_of(&node, &p, "nube", &id).await,
        first,
        "outside the window the repair DID hold; this is new work"
    );
}

// --- Reconcile / backfill -----------------------------------------------------------------------

/// The reconcile pass is the backstop AND the migration: every open, ungrouped insight gets a case,
/// its assignee is carried across, and its comment thread is replayed into the case history with the
/// ORIGINAL author and timestamp. A second pass returns `0`.
///
/// The fixture is built the way an upgrade really produces one — the insight rows are written
/// directly through the real `lb_insights` writers, exactly as a node running before the case plane
/// existed would have left them, so there is no case to find.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn reconcile_backfills_a_case_its_assignee_and_its_comment_thread() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    // A pre-case-plane insight: raised through the crate, so no grouping ever ran.
    let outcome = lb_insights::raise(
        &node.store,
        "nube",
        legacy_raise(
            "legacy-1",
            lb_insights::Severity::Warning,
            "a finding from before the case plane",
        ),
        100,
    )
    .await
    .expect("legacy raise ok");
    let id = outcome.id.clone();
    lb_insights::assign(&node.store, "nube", &id, Some("user:priya"))
        .await
        .expect("legacy assign ok");
    lb_insights::append_comment(
        &node.store,
        "nube",
        &id,
        "attended site",
        "user:priya",
        1_100,
    )
    .await
    .expect("legacy comment ok");
    lb_insights::append_comment(
        &node.store,
        "nube",
        &id,
        "waiting on the PO",
        "user:test",
        1_200,
    )
    .await
    .expect("legacy comment ok");

    assert_eq!(
        open_case_count(&node, &p, "nube").await,
        0,
        "the legacy finding has no case yet — the state the backfill exists for"
    );

    let grouped = lb_host::reconcile_cases(&node, "nube", 2_000)
        .await
        .expect("reconcile ok");
    assert_eq!(grouped, 1);
    assert_eq!(open_case_count(&node, &p, "nube").await, 1);

    let case_id = case_of(&node, &p, "nube", &id).await;
    let case = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert_eq!(case["grouping"], "single");
    assert_eq!(
        case["assigned_to"], "user:priya",
        "the insight's owner must be carried onto the case: {case}"
    );

    // The thread came across — oldest first, original author, original timestamp.
    let events = call(
        &node,
        &p,
        "nube",
        "case.events",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("events ok");
    let comments: Vec<&Value> = events["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["kind"] == "comment")
        .collect();
    assert_eq!(comments.len(), 2, "both notes copied: {events}");
    let attended = comments
        .iter()
        .find(|e| e["data"]["text"] == "attended site")
        .unwrap();
    assert_eq!(
        attended["actor"], "user:priya",
        "a migration must not restamp the author: {attended}"
    );
    assert_eq!(
        attended["ts"], 1_100,
        "nor the timestamp — that would destroy the record it was preserving"
    );

    // IDEMPOTENT: a second pass groups nothing and does not duplicate the thread.
    let again = lb_host::reconcile_cases(&node, "nube", 3_000)
        .await
        .expect("reconcile ok");
    assert_eq!(again, 0, "a second pass must be a no-op");
    let events2 = call(
        &node,
        &p,
        "nube",
        "case.events",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("events ok");
    let n2 = events2["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["kind"] == "comment")
        .count();
    assert_eq!(n2, 2, "the thread must not be copied twice");
    assert_eq!(open_case_count(&node, &p, "nube").await, 1);
}

/// A RESOLVED insight is not open work, so the reconcile pass leaves it alone — no case is minted
/// for a finding nobody is working.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn reconcile_leaves_a_resolved_insight_ungrouped() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    let outcome = lb_insights::raise(
        &node.store,
        "nube",
        legacy_raise(
            "legacy-resolved",
            lb_insights::Severity::Info,
            "already done",
        ),
        100,
    )
    .await
    .expect("raise ok");
    lb_insights::resolve(&node.store, "nube", &outcome.id, "user:test", None, 1_500)
        .await
        .expect("resolve ok");

    let grouped = lb_host::reconcile_cases(&node, "nube", 2_000)
        .await
        .expect("reconcile ok");
    assert_eq!(grouped, 0);
    assert_eq!(open_case_count(&node, &p, "nube").await, 0);
}
