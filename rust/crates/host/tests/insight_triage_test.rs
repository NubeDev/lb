//! The **triage plane** — `assigned_to` + the append-only comment thread — over a REAL booted
//! `Node` (`docs/scope/insights/insight-triage-scope.md`). Real store, real bus, real caps, the real
//! `call_tool` MCP bridge. NO mocks (CLAUDE §9): every record is seeded by raising through the verb
//! under test and read back through it, and memberships/teams are seeded as real rows through the
//! real `lb_authz` writers.
//!
//! Mandatory categories: capability-deny (including the one this scope exists to create — a
//! PRODUCER token gets zero triage write power) + workspace-isolation, both asserting the property
//! only the OUTER gate has: a real id and a fictional id produce **identical** errors.
//!
//! Scope-named cases: dedup preservation incl. the re-open arm (§1), `raise` cannot write triage
//! state (§2), assign semantics + membership validation (§3), un-forgeable author (§4), the list
//! filter incl. `"me"` resolving through TEAMS (§5), the get/list boundary (§6), the caps that
//! REFUSE rather than evict (§7), and comments dying with their insight (§8).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_authz::{membership_add_raw, team_create, MEMBER};
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

const RAISE: &str = "mcp:insight.raise:call";
const GET: &str = "mcp:insight.get:call";
const LIST: &str = "mcp:insight.list:call";
const ASSIGN: &str = "mcp:insight.assign:call";
const COMMENT: &str = "mcp:insight.comment:call";
const RESOLVE: &str = "mcp:insight.resolve:call";
const DELETE: &str = "mcp:insight.delete:call";

/// Every triage cap — the "fully-empowered operator" token most cases use.
const ALL: &[&str] = &[RAISE, GET, LIST, ASSIGN, COMMENT, RESOLVE, DELETE];

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
        "title": "Chullora — no water usage in 1 day",
        "origin": { "kind": "rule", "ref": "rule:no-water-1d" },
        "ts": ts,
    })
}

/// Seed a real workspace roster: `user:test` + `user:priya` are members, `team:mechanical` exists
/// with priya on it. Real rows through the real writers — no fixtures (CLAUDE §9).
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

/// Raise one insight and return its id.
async fn seed_insight(node: &Arc<Node>, p: &Principal, ws: &str, key: &str, ts: u64) -> String {
    let out = call(node, p, ws, "insight.raise", raise_input(key, ts))
        .await
        .expect("raise ok");
    out["id"].as_str().unwrap().to_string()
}

// --- MANDATORY: capability deny ---------------------------------------------------------------

/// The deny this scope EXISTS to create: a producer token holding `mcp:insight.raise:call` and
/// nothing else has **zero** triage write power. If `insight.update` had been the design, this test
/// could not be written — which is the argument for two narrow verbs, in executable form.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_producer_grant_buys_no_triage_write_power() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let full = principal("user:test", "nube", ALL);
    let id = seed_insight(&node, &full, "nube", "k1", 1).await;

    // A pure producer — it may raise, and that is all.
    let producer = principal("key:nightly-rule", "nube", &[RAISE]);
    assert!(
        matches!(
            call(
                &node,
                &producer,
                "nube",
                "insight.assign",
                json!({ "id": id, "assignee": "user:priya" })
            )
            .await,
            Err(ToolError::Denied)
        ),
        "raise grant must not buy assign"
    );
    assert!(
        matches!(
            call(
                &node,
                &producer,
                "nube",
                "insight.comment",
                json!({ "id": id, "text": "mine now", "ts": 2 })
            )
            .await,
            Err(ToolError::Denied)
        ),
        "raise grant must not buy comment"
    );

    // And the record is untouched — the deny happened before any write.
    let got = call(&node, &full, "nube", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert!(got.get("assigned_to").is_none(), "still unassigned");
    assert_eq!(got["comments"].as_array().unwrap().len(), 0, "no thread");
}

/// The property only the OUTER gate has: a denied caller cannot distinguish a REAL id from a
/// fictional one. If these two errors ever differ, the deny has moved inside the verb and become an
/// existence oracle.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn deny_is_identical_for_a_real_id_and_a_fictional_one() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let full = principal("user:test", "nube", ALL);
    let real = seed_insight(&node, &full, "nube", "k1", 1).await;

    // A reader: may look, may not touch.
    let reader = principal("user:bob", "nube", &[GET, LIST]);
    for tool in ["insight.assign", "insight.comment"] {
        let args =
            |id: &str| json!({ "id": id, "assignee": "user:priya", "text": "note", "ts": 2 });
        let on_real = call(&node, &reader, "nube", tool, args(&real)).await;
        let on_fake = call(&node, &reader, "nube", tool, args("no-such-id-at-all")).await;
        assert_eq!(
            format!("{on_real:?}"),
            format!("{on_fake:?}"),
            "{tool}: a real id and a fictional one must deny IDENTICALLY"
        );
        assert!(matches!(on_real, Err(ToolError::Denied)));
    }
}

// --- MANDATORY: workspace isolation -----------------------------------------------------------

/// ws-B cannot assign, comment on, read the thread of, or list a ws-A insight — with identical
/// errors for "another workspace's REAL id" and "an id that exists nowhere".
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_cannot_reach_ws_a_triage_state() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "ws-a").await;
    seed_roster(&node, "ws-b").await;
    let a = principal("user:test", "ws-a", ALL);
    let b = principal("user:test", "ws-b", ALL);

    let a_id = seed_insight(&node, &a, "ws-a", "k1", 1).await;
    call(
        &node,
        &a,
        "ws-a",
        "insight.assign",
        json!({ "id": a_id, "assignee": "user:priya" }),
    )
    .await
    .expect("assign ok in ws-a");
    call(
        &node,
        &a,
        "ws-a",
        "insight.comment",
        json!({ "id": a_id, "text": "ws-a private note", "ts": 2 }),
    )
    .await
    .expect("comment ok in ws-a");

    // Writes: ws-B sees ws-A's real id exactly as it sees a fictional one.
    //
    // Compared with the caller's OWN id redacted out of the message. The two errors are not
    // byte-identical — each echoes back the id that was passed — but that echo is the caller's own
    // input, not a fact learned from the store, so it carries no information about ws-A. The
    // property that must hold is that the error TEMPLATE is the same: ws-B cannot tell "this id is
    // real in another workspace" from "this id exists nowhere".
    for tool in ["insight.assign", "insight.comment"] {
        let args = |id: &str| json!({ "id": id, "assignee": "user:priya", "text": "x", "ts": 3 });
        let on_a = call(&node, &b, "ws-b", tool, args(&a_id)).await;
        let on_fake = call(&node, &b, "ws-b", tool, args("nonexistent")).await;
        let redact = |r: &Result<Value, ToolError>, id: &str| format!("{r:?}").replace(id, "<ID>");
        assert_eq!(
            redact(&on_a, &a_id),
            redact(&on_fake, "nonexistent"),
            "{tool}: ws-A's real id must be indistinguishable from a nonexistent one"
        );
        assert!(
            on_a.is_err(),
            "{tool}: ws-B write on a ws-A insight refused"
        );
    }

    // Reads: no record, and above all no thread.
    let got = call(&node, &b, "ws-b", "insight.get", json!({ "id": a_id }))
        .await
        .expect("get returns");
    assert!(got.is_null(), "ws-B reads no ws-A record: {got}");

    let page = call(&node, &b, "ws-b", "insight.list", json!({}))
        .await
        .expect("list ok");
    assert_eq!(
        page["items"].as_array().unwrap().len(),
        0,
        "ws-B's roster is empty"
    );

    // ws-A still has both, unharmed by the probing.
    let a_got = call(&node, &a, "ws-a", "insight.get", json!({ "id": a_id }))
        .await
        .expect("get ok");
    assert_eq!(a_got["assigned_to"], "user:priya");
    assert_eq!(a_got["comments"].as_array().unwrap().len(), 1);
}

// --- §1 dedup preservation: THE load-bearing regression test ----------------------------------

/// Assign + comment, then re-raise twice: `assigned_to` and every comment survive, `count` advanced.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_re_raise_never_touches_the_owner_or_the_thread() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let id = seed_insight(&node, &p, "nube", "rule:no-water-1d:WM-CHU-01", 1).await;

    call(
        &node,
        &p,
        "nube",
        "insight.assign",
        json!({ "id": id, "assignee": "user:priya" }),
    )
    .await
    .expect("assign ok");
    call(
        &node,
        &p,
        "nube",
        "insight.comment",
        json!({ "id": id, "text": "Site shut for the long weekend — confirming with facilities.", "ts": 2 }),
    )
    .await
    .expect("comment ok");

    // The flapping sensor fires twice more on the same dedup key.
    for ts in [3, 4] {
        call(
            &node,
            &p,
            "nube",
            "insight.raise",
            raise_input("rule:no-water-1d:WM-CHU-01", ts),
        )
        .await
        .expect("re-raise ok");
    }

    let got = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert_eq!(got["count"], 3, "lifetime count advanced");
    assert_eq!(
        got["assigned_to"], "user:priya",
        "a re-raise must NEVER un-assign the technician who took the job"
    );
    let thread = got["comments"].as_array().unwrap();
    assert_eq!(thread.len(), 1, "the note survived every re-raise");
    assert!(thread[0]["text"].as_str().unwrap().starts_with("Site shut"));
}

/// The harder arm the scope names: resolve → re-raise (the RE-OPEN path). `status_by`/`status_ts`
/// clear (a fresh lifecycle) but `assigned_to` and the thread are intact — the fault came back and
/// it is still Priya's, and last time's false-alarm note is what the next responder reads first.
///
/// **This is the #1 revert-check.** See the revert-check note in the session log.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_re_open_arm_clears_the_lifecycle_but_not_the_human_facts() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let key = "rule:no-water-1d:WM-CHU-01";
    let id = seed_insight(&node, &p, "nube", key, 1).await;

    call(
        &node,
        &p,
        "nube",
        "insight.assign",
        json!({ "id": id, "assignee": "user:priya" }),
    )
    .await
    .expect("assign ok");
    call(
        &node,
        &p,
        "nube",
        "insight.comment",
        json!({ "id": id, "text": "False alarm last time: site was shut.", "ts": 2 }),
    )
    .await
    .expect("comment ok");
    call(
        &node,
        &p,
        "nube",
        "insight.resolve",
        json!({ "id": id, "ts": 3 }),
    )
    .await
    .expect("resolve ok");

    // Confirm the resolve actually stamped the lifecycle, or the assertion below proves nothing.
    let resolved = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert_eq!(resolved["status"], "resolved");
    assert_eq!(resolved["status_by"], "user:test");

    // Two weeks later the meter genuinely fails — same key, so this is the re-open arm.
    call(&node, &p, "nube", "insight.raise", raise_input(key, 4))
        .await
        .expect("re-raise ok");

    let got = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert_eq!(got["status"], "open", "re-opened");
    assert!(
        got.get("status_by").is_none() || got["status_by"].is_null(),
        "the LIFECYCLE clears on re-open: {got}"
    );
    assert!(
        got.get("status_ts").is_none() || got["status_ts"].is_null(),
        "the LIFECYCLE clears on re-open: {got}"
    );
    assert_eq!(
        got["assigned_to"], "user:priya",
        "the HUMAN FACT does not clear on re-open — this is the decision most likely to be \
         'cleaned up' beside status_by; see the SCOPE comment on the re-open arm in raise.rs"
    );
    assert_eq!(
        got["comments"].as_array().unwrap().len(),
        1,
        "and the note explaining last time's false alarm is still there"
    );
}

// --- §2 raise cannot write triage state -------------------------------------------------------

/// A hostile producer putting `assigned_to`/`comments` in the raise body reaches nothing: the fields
/// are absent from `RaiseInput`, so this is a serde-level guarantee, not a runtime filter.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_raise_body_carrying_triage_fields_is_ignored() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    let mut input = raise_input("k1", 1);
    input["assigned_to"] = json!("user:attacker");
    input["comments"] = json!([{ "seq": 1, "text": "injected", "author": "user:victim", "ts": 1 }]);
    let out = call(&node, &p, "nube", "insight.raise", input)
        .await
        .expect("raise still succeeds — the extra keys are simply not part of the input shape");
    let id = out["id"].as_str().unwrap();

    let got = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert!(
        got.get("assigned_to").is_none(),
        "a producer cannot reach the owner axis: {got}"
    );
    assert_eq!(
        got["comments"].as_array().unwrap().len(),
        0,
        "a producer cannot seed the thread: {got}"
    );
}

// --- §3 assign semantics + membership validation ----------------------------------------------

/// Assign → re-assign → un-assign round-trips; assigning the current assignee is an idempotent
/// no-op; a `team:` subject is legal from v1.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn assign_reassign_unassign_round_trips_and_is_idempotent() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let id = seed_insight(&node, &p, "nube", "k1", 1).await;

    let assign = |assignee: Value| {
        let id = id.clone();
        let node = node.clone();
        let p = p.clone();
        async move {
            call(
                &node,
                &p,
                "nube",
                "insight.assign",
                json!({ "id": id, "assignee": assignee }),
            )
            .await
        }
    };
    let owner = |got: &Value| got.get("assigned_to").cloned().unwrap_or(Value::Null);

    assign(json!("user:priya")).await.expect("assign ok");
    let got = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .unwrap();
    assert_eq!(owner(&got), json!("user:priya"));

    // Idempotent: the same assignee again is a no-op success.
    assign(json!("user:priya")).await.expect("idempotent");
    let got = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .unwrap();
    assert_eq!(owner(&got), json!("user:priya"));

    // Re-assign to a TEAM — legal from v1 (queue-style ownership).
    assign(json!("team:mechanical")).await.expect("team assign");
    let got = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .unwrap();
    assert_eq!(owner(&got), json!("team:mechanical"));

    // Un-assign.
    assign(Value::Null).await.expect("un-assign ok");
    let got = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .unwrap();
    assert_eq!(owner(&got), Value::Null, "back in the triage queue");
}

/// Membership validation (resolved decision 2), and the opacity that makes it safe: a non-member, a
/// nonexistent subject, and a REAL member of another workspace all produce the SAME error — so a
/// probe cannot confirm that `user:zoe` exists in ws-B.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_assignee_who_is_not_a_member_here_is_refused_opaquely() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    // A real member of a DIFFERENT workspace — the case a probe would use.
    membership_add_raw(&node.store, "other", "user:zoe", 1)
        .await
        .unwrap();
    team_create(&node.store, "other", "team:electrical", "Electrical")
        .await
        .unwrap();

    let p = principal("user:test", "nube", ALL);
    let id = seed_insight(&node, &p, "nube", "k1", 1).await;

    let attempt = |assignee: &str| {
        let id = id.clone();
        let node = node.clone();
        let p = p.clone();
        let assignee = assignee.to_string();
        async move {
            call(
                &node,
                &p,
                "nube",
                "insight.assign",
                json!({ "id": id, "assignee": assignee }),
            )
            .await
        }
    };

    let nonexistent = attempt("user:nobody-at-all").await;
    let other_ws_user = attempt("user:zoe").await;
    let other_ws_team = attempt("team:electrical").await;
    let not_a_subject = attempt("priya").await;

    assert!(nonexistent.is_err(), "a nonexistent subject is refused");
    assert_eq!(
        format!("{other_ws_user:?}"),
        format!("{nonexistent:?}"),
        "a REAL member of another workspace must be indistinguishable from a subject that does \
         not exist — otherwise assign is a cross-tenant existence oracle"
    );
    assert_eq!(
        format!("{other_ws_team:?}"),
        format!("{nonexistent:?}"),
        "same for a real team of another workspace"
    );
    assert!(not_a_subject.is_err(), "a bare string is not a subject");

    // And nothing was written by any of the refused attempts.
    let got = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .unwrap();
    assert!(got.get("assigned_to").is_none(), "still unassigned: {got}");
}

/// Assigning a nonexistent insight errors like `ack` does.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn assigning_a_missing_insight_errors() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let out = call(
        &node,
        &p,
        "nube",
        "insight.assign",
        json!({ "id": "nope", "assignee": "user:priya" }),
    )
    .await;
    assert!(out.is_err(), "no such insight");
}

/// Bulk assign: per-item results, the cap REPORTED rather than silently truncating.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn bulk_assign_reports_per_item_results_and_never_truncates_silently() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    let mut ids = Vec::new();
    for i in 0..3 {
        ids.push(seed_insight(&node, &p, "nube", &format!("k{i}"), 1).await);
    }
    // One id that isn't real — the partial-failure case a green toast would hide.
    ids.push("not-a-real-id".to_string());

    let out = call(
        &node,
        &p,
        "nube",
        "insight.assign",
        json!({ "ids": ids, "assignee": "user:priya" }),
    )
    .await
    .expect("bulk call itself succeeds");
    let results = out["results"].as_array().unwrap();
    assert_eq!(
        results.len(),
        4,
        "one result per id, not a rolled-up status"
    );
    assert_eq!(results.iter().filter(|r| r["ok"] == true).count(), 3);
    let failed: Vec<_> = results.iter().filter(|r| r["ok"] == false).collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["id"], "not-a-real-id");
    assert!(
        failed[0]["error"].as_str().is_some(),
        "the failure carries a reason the UI can surface"
    );

    // The 3 real ones really were assigned — a partial failure did not roll back the successes.
    for id in ids.iter().take(3) {
        let got = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
            .await
            .unwrap();
        assert_eq!(got["assigned_to"], "user:priya");
    }

    // Over the cap is an EXPLICIT error — never the first 100 silently.
    let too_many: Vec<String> = (0..lb_host::MAX_BULK_ASSIGN + 1)
        .map(|i| format!("id-{i}"))
        .collect();
    let err = call(
        &node,
        &p,
        "nube",
        "insight.assign",
        json!({ "ids": too_many, "assignee": "user:priya" }),
    )
    .await;
    match err {
        Err(ToolError::BadInput(m)) => assert!(
            m.contains(&lb_host::MAX_BULK_ASSIGN.to_string()),
            "the cap is REPORTED: {m}"
        ),
        other => panic!("expected an explicit cap error, got {other:?}"),
    }
}

// --- §4 the author is un-forgeable ------------------------------------------------------------

/// A comment body supplying `author` stores the PRINCIPAL's sub instead (the `ack.rs` host-stamp
/// precedent) — a caller cannot forge another operator's note.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_comment_author_is_host_stamped_never_caller_supplied() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let id = seed_insight(&node, &p, "nube", "k1", 1).await;

    call(
        &node,
        &p,
        "nube",
        "insight.comment",
        json!({ "id": id, "text": "note", "author": "user:someone-else", "ts": 2 }),
    )
    .await
    .expect("comment ok");

    let got = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .unwrap();
    assert_eq!(
        got["comments"][0]["author"], "user:test",
        "the author is the principal, not what the body claimed"
    );
}

// --- §5 the list filter -----------------------------------------------------------------------

/// `"none"` returns exactly the unassigned; an explicit subject returns only that subject's; and
/// `"me"` resolves to the principal **and their teams** — the case a naive sub-equality check
/// silently drops.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_assigned_to_filter_resolves_none_a_subject_and_me_including_teams() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let test = principal("user:test", "nube", ALL);
    // priya is on `team:mechanical` (seeded above); her token drives the "me" view.
    let priya = principal("user:priya", "nube", ALL);

    let unassigned = seed_insight(&node, &test, "nube", "k-unassigned", 1).await;
    let to_priya = seed_insight(&node, &test, "nube", "k-priya", 2).await;
    let to_team = seed_insight(&node, &test, "nube", "k-team", 3).await;
    let to_test = seed_insight(&node, &test, "nube", "k-test", 4).await;

    for (id, assignee) in [
        (&to_priya, "user:priya"),
        (&to_team, "team:mechanical"),
        (&to_test, "user:test"),
    ] {
        call(
            &node,
            &test,
            "nube",
            "insight.assign",
            json!({ "id": id, "assignee": assignee }),
        )
        .await
        .expect("assign ok");
    }

    let ids = |page: &Value| {
        let mut v: Vec<String> = page["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["id"].as_str().unwrap().to_string())
            .collect();
        v.sort();
        v
    };

    // "none" — the triage queue.
    let page = call(
        &node,
        &test,
        "nube",
        "insight.list",
        json!({ "assigned_to": "none" }),
    )
    .await
    .unwrap();
    assert_eq!(
        ids(&page),
        vec![unassigned.clone()],
        "exactly the unassigned"
    );

    // An explicit subject.
    let page = call(
        &node,
        &test,
        "nube",
        "insight.list",
        json!({ "assigned_to": "user:priya" }),
    )
    .await
    .unwrap();
    assert_eq!(ids(&page), vec![to_priya.clone()]);

    // "me" for priya = her own sub PLUS team:mechanical. The team row is the load-bearing one.
    let page = call(
        &node,
        &priya,
        "nube",
        "insight.list",
        json!({ "assigned_to": "me" }),
    )
    .await
    .unwrap();
    let mut expected = vec![to_priya.clone(), to_team.clone()];
    expected.sort();
    assert_eq!(
        ids(&page),
        expected,
        "'me' must include TEAM-assigned findings — a naive sub-equality check drops the queue \
         the team-subject decision exists to support"
    );

    // "me" for test = only her own (she is not on the crew).
    let page = call(
        &node,
        &test,
        "nube",
        "insight.list",
        json!({ "assigned_to": "me" }),
    )
    .await
    .unwrap();
    assert_eq!(ids(&page), vec![to_test.clone()]);
}

/// The owner filter composes with the other axes and with keyset paging without breaking the cursor.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_owner_filter_composes_with_status_and_keyset_paging() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    // 5 insights assigned to priya, 3 to nobody; resolve one of priya's.
    let mut priya_ids = Vec::new();
    for i in 0..5 {
        let id = seed_insight(&node, &p, "nube", &format!("p{i}"), 10 + i).await;
        call(
            &node,
            &p,
            "nube",
            "insight.assign",
            json!({ "id": id, "assignee": "user:priya" }),
        )
        .await
        .unwrap();
        priya_ids.push(id);
    }
    for i in 0..3 {
        seed_insight(&node, &p, "nube", &format!("u{i}"), 20 + i).await;
    }
    call(
        &node,
        &p,
        "nube",
        "insight.resolve",
        json!({ "id": priya_ids[0], "ts": 99 }),
    )
    .await
    .unwrap();

    // Composed with status: 4 of priya's 5 are still open.
    let page = call(
        &node,
        &p,
        "nube",
        "insight.list",
        json!({ "assigned_to": "user:priya", "status": "open" }),
    )
    .await
    .unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 4);

    // Paged: 2 + 2 + end, every row priya's, no duplicates across the boundary.
    let mut seen: Vec<String> = Vec::new();
    let mut cursor = Value::Null;
    loop {
        let mut q = json!({ "assigned_to": "user:priya", "limit": 2 });
        if !cursor.is_null() {
            q["cursor"] = cursor.clone();
        }
        let page = call(&node, &p, "nube", "insight.list", q).await.unwrap();
        for item in page["items"].as_array().unwrap() {
            assert_eq!(item["assigned_to"], "user:priya", "every paged row is hers");
            seen.push(item["id"].as_str().unwrap().to_string());
        }
        match page.get("next") {
            Some(n) if !n.is_null() => cursor = n.clone(),
            _ => break,
        }
    }
    seen.sort();
    let mut expected = priya_ids.clone();
    expected.sort();
    assert_eq!(seen, expected, "paging returned each of hers exactly once");
}

// --- §6 the get / list boundary ---------------------------------------------------------------

/// `insight.list` carries the scalar `assigned_to` (the owner column) but NEVER `comments`;
/// `insight.get` carries the full thread, newest-first.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_carries_the_owner_column_but_never_the_thread() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let id = seed_insight(&node, &p, "nube", "k1", 1).await;

    call(
        &node,
        &p,
        "nube",
        "insight.assign",
        json!({ "id": id, "assignee": "user:priya" }),
    )
    .await
    .unwrap();
    for (i, text) in ["first note", "second note", "third note"]
        .iter()
        .enumerate()
    {
        call(
            &node,
            &p,
            "nube",
            "insight.comment",
            json!({ "id": id, "text": text, "ts": 10 + i as u64 }),
        )
        .await
        .unwrap();
    }

    let page = call(&node, &p, "nube", "insight.list", json!({}))
        .await
        .unwrap();
    let row = &page["items"][0];
    assert_eq!(
        row["assigned_to"], "user:priya",
        "the roster renders an owner column with no N+1"
    );
    assert!(
        row.get("comments").is_none(),
        "the thread NEVER rides the roster: {row}"
    );

    let got = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .unwrap();
    let thread = got["comments"].as_array().unwrap();
    assert_eq!(thread.len(), 3, "the drawer gets the COMPLETE thread");
    assert_eq!(
        thread[0]["text"], "third note",
        "newest-first — the last thing anyone learned is the first thing they need"
    );
    assert_eq!(thread[2]["text"], "first note");
    // seq is monotone per insight and stable (nothing evicts, nothing is deleted).
    assert_eq!(thread[0]["cseq"], 3);
    assert_eq!(thread[2]["cseq"], 1);
}

// --- §7 comment caps REFUSE, never evict ------------------------------------------------------

/// An oversize `text` rejects the whole call before any write — never a silent truncation.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_oversize_comment_is_refused_whole_and_writes_nothing() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let id = seed_insight(&node, &p, "nube", "k1", 1).await;

    call(
        &node,
        &p,
        "nube",
        "insight.comment",
        json!({ "id": id, "text": "a real note", "ts": 2 }),
    )
    .await
    .unwrap();

    let huge = "x".repeat(lb_insights::MAX_COMMENT_BYTES + 1);
    let err = call(
        &node,
        &p,
        "nube",
        "insight.comment",
        json!({ "id": id, "text": huge, "ts": 3 }),
    )
    .await;
    assert!(matches!(err, Err(ToolError::BadInput(_))), "refused loudly");

    let got = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .unwrap();
    let thread = got["comments"].as_array().unwrap();
    assert_eq!(thread.len(), 1, "nothing partial was written");
    assert_eq!(
        thread[0]["text"], "a real note",
        "and no truncated version of the oversize note landed"
    );

    // Empty is refused too — an empty note is indistinguishable from a mis-click.
    assert!(call(
        &node,
        &p,
        "nube",
        "insight.comment",
        json!({ "id": id, "text": "   ", "ts": 4 })
    )
    .await
    .is_err());
}

/// **The decision most likely to be reverted to match the occurrence ring beside it.** Appending
/// past the per-insight count cap ERRORS, and the pre-existing thread is unchanged — the OLDEST
/// comment is still there after the refused write. If someone swaps in ring eviction, this goes red.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_count_cap_refuses_the_write_it_does_not_evict_the_oldest() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let id = seed_insight(&node, &p, "nube", "k1", 1).await;

    // Fill the thread exactly to the cap.
    for i in 0..lb_insights::MAX_COMMENTS_PER_INSIGHT {
        call(
            &node,
            &p,
            "nube",
            "insight.comment",
            json!({ "id": id, "text": format!("note {i}"), "ts": 10 + i as u64 }),
        )
        .await
        .unwrap_or_else(|e| panic!("comment {i} ok: {e:?}"));
    }

    let err = call(
        &node,
        &p,
        "nube",
        "insight.comment",
        json!({ "id": id, "text": "one too many", "ts": 9_999 }),
    )
    .await;
    match err {
        Err(ToolError::BadInput(m)) => assert!(
            m.contains(&lb_insights::MAX_COMMENTS_PER_INSIGHT.to_string()),
            "the cap is named in the refusal: {m}"
        ),
        other => panic!("the count cap must REFUSE, not evict — got {other:?}"),
    }

    let got = call(&node, &p, "nube", "insight.get", json!({ "id": id }))
        .await
        .unwrap();
    let thread = got["comments"].as_array().unwrap();
    assert_eq!(
        thread.len(),
        lb_insights::MAX_COMMENTS_PER_INSIGHT,
        "the thread is unchanged by the refused write"
    );
    // THE assertion: eviction would have dropped this one to make room.
    assert!(
        thread.iter().any(|c| c["text"] == "note 0"),
        "the OLDEST comment survives a refused write — comments are not a ring (resolved \
         decision 4): 'we wrote it down and the platform deleted it' is a trust failure"
    );
    assert!(
        !thread.iter().any(|c| c["text"] == "one too many"),
        "and the refused note is not in the thread"
    );
}

// --- §8 comments die WITH their insight, not before --------------------------------------------

/// Delete the insight → its comments are gone (no orphan rows accumulating outside any retention
/// sweep). Verified through a fresh raise of the SAME dedup key, which mints a new id: the old
/// thread must not reappear under it, and a direct read of the thread is empty.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn deleting_an_insight_takes_its_comments_with_it() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);
    let id = seed_insight(&node, &p, "nube", "k1", 1).await;

    for i in 0..3 {
        call(
            &node,
            &p,
            "nube",
            "insight.comment",
            json!({ "id": id, "text": format!("note {i}"), "ts": 10 + i }),
        )
        .await
        .unwrap();
    }
    // The thread really is there before the delete, or the assertion after it proves nothing.
    assert_eq!(
        lb_insights::comments(&node.store, "nube", &id)
            .await
            .unwrap()
            .len(),
        3
    );

    call(&node, &p, "nube", "insight.delete", json!({ "id": id }))
        .await
        .expect("delete ok");

    assert!(
        lb_insights::comments(&node.store, "nube", &id)
            .await
            .unwrap()
            .is_empty(),
        "comments are purged WITH their parent — they have no retention schedule of their own, \
         so a cascade that missed them would leave human notes as permanent orphans"
    );

    // A long-lived insight, by contrast, keeps its oldest comment indefinitely: re-raise the same
    // key many times and note 0 is still the oldest row.
    let fresh = seed_insight(&node, &p, "nube", "k1", 50).await;
    assert_ne!(fresh, id, "the delete really removed the old record");
    assert!(
        lb_insights::comments(&node.store, "nube", &fresh)
            .await
            .unwrap()
            .is_empty(),
        "the new record starts with an empty thread — no resurrected orphans"
    );
    call(
        &node,
        &p,
        "nube",
        "insight.comment",
        json!({ "id": fresh, "text": "oldest", "ts": 51 }),
    )
    .await
    .unwrap();
    for ts in 52..60 {
        call(&node, &p, "nube", "insight.raise", raise_input("k1", ts))
            .await
            .unwrap();
    }
    let thread = lb_insights::comments(&node.store, "nube", &fresh)
        .await
        .unwrap();
    assert_eq!(thread.len(), 1);
    assert_eq!(thread[0].text, "oldest", "kept for the life of the insight");
}
