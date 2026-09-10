//! `insight.assign` / `insight.comment` as **delegates** of the case that owns the work
//! (`docs/scope/insights/case-plane-scope.md`, resolved decision 5), over a REAL booted `Node`.
//!
//! The thing under test is a promise about compatibility, not a new feature: the shipped verb
//! surface, the caps and the return shapes must be **exactly** what they were, while the durable
//! answer moves to the case. So every assertion here is either "the old contract still holds" or
//! "the fact landed on the case".
//!
//! The one behaviour that legitimately changed — and it is the point of the plane — is that
//! assigning ONE detection of a fault assigns every detection its case cites. Three symptoms of one
//! chiller fault are one job with one owner, not three people's work. That is asserted explicitly
//! rather than left to be discovered.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_authz::{membership_add_raw, team_create, MEMBER};
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

const RAISE: &str = "mcp:insight.raise:call";
const I_GET: &str = "mcp:insight.get:call";
const I_LIST: &str = "mcp:insight.list:call";
const I_ASSIGN: &str = "mcp:insight.assign:call";
const I_COMMENT: &str = "mcp:insight.comment:call";
const GET: &str = "mcp:case.get:call";
const LIST: &str = "mcp:case.list:call";

const ALL: &[&str] = &[RAISE, I_GET, I_LIST, I_ASSIGN, I_COMMENT, GET, LIST];

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

async fn seed_insight(node: &Arc<Node>, p: &Principal, ws: &str, key: &str, ts: u64) -> String {
    call(node, p, ws, "insight.raise", raise_input(key, ts))
        .await
        .expect("raise ok")["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn case_of(node: &Arc<Node>, p: &Principal, ws: &str, insight_id: &str) -> String {
    call(node, p, ws, "insight.get", json!({ "id": insight_id }))
        .await
        .expect("get ok")["case_id"]
        .as_str()
        .unwrap()
        .to_string()
}

// --- The delegation, both directions ------------------------------------------------------------

/// `insight.assign` writes the CASE and echoes onto the insight. The return shape
/// (`{ assigned_to }`) and the cap are unchanged, and `insight.get`/`insight.list` still answer from
/// the insight — which is what keeps every shipped reader working.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn insight_assign_writes_the_case_and_echoes_back_onto_the_insight() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let id = seed_insight(&node, &p, "nube", "deleg-a", 1).await;
    let case_id = case_of(&node, &p, "nube", &id).await;

    // The SHIPPED shape: `{ assigned_to }` for a single-id call.
    let out = call(
        &node,
        &p,
        "nube",
        "insight.assign",
        json!({ "id": id, "assignee": "team:mechanical", "ts": 2 }),
    )
    .await
    .expect("assign ok");
    assert_eq!(
        out,
        json!({ "assigned_to": "team:mechanical" }),
        "the return shape must not change"
    );

    // The CASE is the authority.
    let case = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert_eq!(case["assigned_to"], "team:mechanical");
    // …and it is in the case's history, attributed to the caller.
    let events = lb_cases::events(&node.store, "nube", &case_id, 50, None)
        .await
        .expect("events ok");
    let assigned = events
        .items
        .iter()
        .find(|e| e.kind == lb_cases::EventKind::Assigned)
        .expect("an `assigned` event");
    assert_eq!(assigned.actor, "user:test");

    // The INSIGHT carries the echo, so every shipped reader still answers.
    let insight = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert_eq!(insight["assigned_to"], "team:mechanical");
    let page = call(
        &node,
        &p,
        "nube",
        "insight.list",
        json!({ "filter": { "assigned_to": "team:mechanical" } }),
    )
    .await
    .expect("list ok");
    assert_eq!(page["items"].as_array().unwrap().len(), 1, "{page}");

    // Un-assign clears both planes.
    let out = call(
        &node,
        &p,
        "nube",
        "insight.assign",
        json!({ "id": id, "ts": 3 }),
    )
    .await
    .expect("un-assign ok");
    assert_eq!(out, json!({ "assigned_to": Value::Null }));
    let case = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert!(case["assigned_to"].is_null(), "{case}");
    let insight = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert!(insight["assigned_to"].is_null(), "{insight}");
}

/// **Assigning one detection assigns the whole job.** The behaviour change the plane exists to
/// produce, asserted rather than discovered: three symptoms of one fault, merged into one case, take
/// one owner together.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn assigning_one_detection_assigns_every_detection_the_case_cites() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal(
        "user:test",
        "nube",
        &[
            RAISE,
            I_GET,
            I_LIST,
            I_ASSIGN,
            GET,
            LIST,
            "mcp:case.open:call",
        ],
    );
    let a = seed_insight(&node, &p, "nube", "fan-a", 1).await;
    let b = seed_insight(&node, &p, "nube", "fan-b", 2).await;
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

    call(
        &node,
        &p,
        "nube",
        "insight.assign",
        json!({ "id": a, "assignee": "user:priya", "ts": 4 }),
    )
    .await
    .expect("assign ok");

    for id in [&a, &b] {
        let insight = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
            .await
            .expect("get ok");
        assert_eq!(
            insight["assigned_to"], "user:priya",
            "every detection the case cites takes the owner: {insight}"
        );
    }
}

/// `insight.comment` returns the insight thread's `seq` as it always did, composes into
/// `insight.get` as it always did, AND lands the note in the case's history.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn insight_comment_keeps_the_thread_and_lands_the_note_on_the_case() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let id = seed_insight(&node, &p, "nube", "deleg-c", 1).await;
    let case_id = case_of(&node, &p, "nube", &id).await;

    let first = call(
        &node,
        &p,
        "nube",
        "insight.comment",
        json!({ "id": id, "text": "attended site", "ts": 2 }),
    )
    .await
    .expect("comment ok");
    assert_eq!(first, json!({ "seq": 1 }), "the shipped `{{seq}}` shape");
    let second = call(
        &node,
        &p,
        "nube",
        "insight.comment",
        json!({ "id": id, "text": "waiting on the PO", "ts": 3 }),
    )
    .await
    .expect("comment ok");
    assert_eq!(second, json!({ "seq": 2 }), "and it still increments");

    // `insight.get` still composes the thread — the drawer's one round-trip.
    let insight = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    let thread = insight["comments"].as_array().unwrap();
    assert_eq!(thread.len(), 2, "{insight}");
    assert_eq!(thread[0]["text"], "waiting on the PO", "newest first");
    assert_eq!(thread[0]["author"], "user:test");

    // And the case history has both, as `comment` events with the same author.
    let events = lb_cases::events(&node.store, "nube", &case_id, 50, None)
        .await
        .expect("events ok");
    let comments: Vec<_> = events
        .items
        .iter()
        .filter(|e| e.kind == lb_cases::EventKind::Comment)
        .collect();
    assert_eq!(comments.len(), 2, "{events:?}");
    assert!(comments.iter().all(|c| c.actor == "user:test"));
}

/// A REFUSED note leaves both planes untouched. The insight thread's bounds are the stricter ones
/// (an empty note, a 4 KB cap, a 200-note count cap that refuses rather than evicting), so the
/// thread runs first — the reverse order would land an oversize note on the case and THEN fail the
/// call, which is the one outcome a refusal must not produce.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_refused_note_lands_on_neither_plane() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let id = seed_insight(&node, &p, "nube", "deleg-d", 1).await;
    let case_id = case_of(&node, &p, "nube", &id).await;

    for bad in ["", &"x".repeat(5000)] {
        let err = call(
            &node,
            &p,
            "nube",
            "insight.comment",
            json!({ "id": id, "text": bad, "ts": 2 }),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ToolError::BadInput(_)), "{err:?}");
    }

    let insight = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert!(
        insight["comments"].as_array().unwrap().is_empty(),
        "the insight thread is untouched: {insight}"
    );
    let events = lb_cases::events(&node.store, "nube", &case_id, 50, None)
        .await
        .expect("events ok");
    assert!(
        !events
            .items
            .iter()
            .any(|e| e.kind == lb_cases::EventKind::Comment),
        "no note reached the case history either: {events:?}"
    );
}

/// **A producer grant still buys zero triage write power.** The deny this scope exists to create,
/// re-asserted after the delegation: if `insight.assign` had quietly started riding a case cap — or
/// dropped its own — this is what would catch it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_producer_grant_still_buys_no_triage_write_power() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let full = principal("user:test", "nube", ALL);
    let id = seed_insight(&node, &full, "nube", "deleg-e", 1).await;

    let producer = principal("key:nightly-rule", "nube", &[RAISE]);
    for (tool, input) in [
        (
            "insight.assign",
            json!({ "id": id, "assignee": "user:priya", "ts": 2 }),
        ),
        (
            "insight.comment",
            json!({ "id": id, "text": "hi", "ts": 2 }),
        ),
    ] {
        let err = call(&node, &producer, "nube", tool, input)
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Denied), "{tool}: {err:?}");
    }

    // Conversely, the triage caps alone are still enough — the delegation did not add a second wall.
    let triager = principal("user:test", "nube", &[I_GET, I_ASSIGN, I_COMMENT]);
    call(
        &node,
        &triager,
        "nube",
        "insight.assign",
        json!({ "id": id, "assignee": "user:priya", "ts": 3 }),
    )
    .await
    .expect("insight.assign must NOT require a case capability");
    call(
        &node,
        &triager,
        "nube",
        "insight.comment",
        json!({ "id": id, "text": "still works", "ts": 4 }),
    )
    .await
    .expect("insight.comment must NOT require a case capability");
}

/// The bulk contract is unchanged: per-item results, an explicit over-cap refusal, and a per-item
/// failure for a missing id rather than a failed call.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn bulk_assign_keeps_its_per_item_contract() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let a = seed_insight(&node, &p, "nube", "bulk-a", 1).await;
    let b = seed_insight(&node, &p, "nube", "bulk-b", 2).await;

    let out = call(
        &node,
        &p,
        "nube",
        "insight.assign",
        json!({ "ids": [a, b, "01NOPE"], "assignee": "user:priya", "ts": 3 }),
    )
    .await
    .expect("bulk assign ok");
    let results = out["results"].as_array().unwrap();
    assert_eq!(results.len(), 3, "{out}");
    assert_eq!(results[0]["ok"], true);
    assert_eq!(results[1]["ok"], true);
    assert_eq!(
        results[2]["ok"], false,
        "a missing id fails per-item: {out}"
    );
    assert!(results[2]["error"]
        .as_str()
        .unwrap()
        .contains("no such insight"));

    // Over the cap the WHOLE call is refused — reported, never silently truncated.
    let too_many: Vec<String> = (0..101).map(|i| format!("id-{i}")).collect();
    let err = call(
        &node,
        &p,
        "nube",
        "insight.assign",
        json!({ "ids": too_many, "assignee": "user:priya", "ts": 4 }),
    )
    .await
    .unwrap_err();
    match err {
        ToolError::BadInput(m) => assert!(m.contains("bulk cap"), "{m}"),
        other => panic!("expected BadInput, got {other:?}"),
    }
}
