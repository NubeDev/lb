//! The self-clear reactor: an untouched case whose insights all cleared is resolved `self_cleared`,
//! under a service policy that opts in (`docs/scope/insights/case-self-clear-scope.md`).
//!
//! Part of the `reactor` suite (see `reactor_support.rs` for the fixtures and the full preamble). One
//! binary: `case_suite.rs`.
//!
//! Like the `reconcile_*` cases, `close_self_cleared_cases` is a node-level pass invoked as a Rust
//! function, not a verb behind the caps wall, so there is no cap to remove. The RED half is a
//! **pre-pass assertion** instead: every test proves the case is OPEN, with its insight already
//! resolved, before the pass runs — so "the pass closed it" (or "the pass left it alone") is a claim
//! the test makes, not one it assumes.

use super::reactor_support::*;

const RESOLVE: &str = "mcp:insight.resolve:call";
const POL_SET: &str = "mcp:policy.sla.set:call";
const COMMENT: &str = "mcp:case.comment:call";
const DAY: u64 = 24 * 60 * 60 * 1000;

fn caps() -> Vec<&'static str> {
    let mut c = ALL.to_vec();
    c.extend([RESOLVE, POL_SET, COMMENT]);
    c
}

/// The workspace default policy, with the opt-in set as given. Installed BEFORE any raise so the
/// SLA clock stamps it on every case (`policy_id`), which is how the pass finds the flag.
async fn install_policy(node: &Arc<Node>, p: &Principal, auto_close: bool) {
    call(
        node,
        p,
        "nube",
        "policy.sla.set",
        json!({
            "id": "default",
            "name": "workspace default",
            "match": {},
            "respond_h": 4,
            "resolve_h": 24,
            "auto_close_self_cleared": auto_close,
        }),
    )
    .await
    .expect("policy set");
}

async fn raise(node: &Arc<Node>, p: &Principal, key: &str, ts: u64) -> String {
    call(node, p, "nube", "insight.raise", raise_input(key, ts))
        .await
        .expect("raise ok")["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn resolve(node: &Arc<Node>, p: &Principal, id: &str, ts: u64) {
    call(
        node,
        p,
        "nube",
        "insight.resolve",
        json!({ "id": id, "ts": ts }),
    )
    .await
    .expect("resolve ok");
}

async fn case(node: &Arc<Node>, p: &Principal, case_id: &str) -> Value {
    call(node, p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("case.get ok")
}

async fn pass(node: &Arc<Node>, now: u64) -> usize {
    lb_host::close_self_cleared_cases(node, "nube", now)
        .await
        .expect("pass ok")
}

/// **The headline.** Insight resolved by its producer, case never touched, policy opted in ⇒ the case
/// is resolved `self_cleared` by `system:self-clear`, and a second pass closes nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_untouched_case_whose_insight_cleared_is_resolved_self_cleared() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", &caps());
    install_policy(&node, &p, true).await;

    let id = raise(&node, &p, "flat-1", DAY).await;
    let case_id = case_of(&node, &p, "nube", &id).await;
    resolve(&node, &p, &id, 2 * DAY).await;

    // RED (pre-pass): the insight is resolved, the case is still open work.
    let before = case(&node, &p, &case_id).await;
    assert_eq!(before["workflow"], "to_action", "{before}");
    assert_eq!(before["closed"], false);
    assert_eq!(
        before["policy_id"], "default",
        "the SLA clock must stamp the policy: {before}"
    );

    assert_eq!(pass(&node, 3 * DAY).await, 1);

    let after = case(&node, &p, &case_id).await;
    assert_eq!(after["workflow"], "resolved", "{after}");
    assert_eq!(after["resolution"], "self_cleared");
    assert_eq!(after["closed"], true);
    assert_eq!(after["resolved_by"], lb_host::SELF_CLEAR_ACTOR);
    assert_eq!(
        open_case_count(&node, &p, "nube").await,
        0,
        "the queue is clear"
    );

    assert_eq!(
        pass(&node, 4 * DAY).await,
        0,
        "idempotent: nothing left to close"
    );
}

/// **Off by default.** The same resolved insight under a policy that has not opted in — or under no
/// policy at all — leaves the case for a person, exactly as before this reactor existed.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn without_the_opt_in_nothing_closes() {
    // No policy at all.
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", &caps());
    let id = raise(&node, &p, "flat-2", DAY).await;
    let case_id = case_of(&node, &p, "nube", &id).await;
    resolve(&node, &p, &id, 2 * DAY).await;
    assert_eq!(pass(&node, 3 * DAY).await, 0);
    assert_eq!(case(&node, &p, &case_id).await["workflow"], "to_action");

    // A policy that says `false`.
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    install_policy(&node, &p, false).await;
    let id = raise(&node, &p, "flat-3", DAY).await;
    let case_id = case_of(&node, &p, "nube", &id).await;
    resolve(&node, &p, &id, 2 * DAY).await;
    assert_eq!(pass(&node, 3 * DAY).await, 0);
    assert_eq!(case(&node, &p, &case_id).await["workflow"], "to_action");
}

/// A case whose insight is still firing is live work — the pass must not touch it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_case_still_firing_stays_open() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", &caps());
    install_policy(&node, &p, true).await;

    let id = raise(&node, &p, "flat-4", DAY).await;
    let case_id = case_of(&node, &p, "nube", &id).await;
    assert_eq!(pass(&node, 3 * DAY).await, 0);
    assert_eq!(case(&node, &p, &case_id).await["workflow"], "to_action");
}

/// **Worked cases stay for a person**: a comment, an assignment, a stage change — each on its own case,
/// each insight resolved — and the pass closes none of them, while an untouched sibling does close
/// (so the zero is the guard working, not the pass being dead).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_worked_case_stays_for_a_person() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", &caps());
    install_policy(&node, &p, true).await;

    let mut worked = Vec::new();
    for (key, action) in [
        ("c-comment", "comment"),
        ("c-assign", "assign"),
        ("c-stage", "stage"),
    ] {
        let id = raise(&node, &p, key, DAY).await;
        let case_id = case_of(&node, &p, "nube", &id).await;
        match action {
            "comment" => call(
                &node,
                &p,
                "nube",
                "case.comment",
                json!({ "id": case_id, "text": "looking into it", "ts": DAY + 1 }),
            ),
            "assign" => call(
                &node,
                &p,
                "nube",
                "case.assign",
                json!({ "id": case_id, "assignee": "user:priya", "ts": DAY + 1 }),
            ),
            _ => call(
                &node,
                &p,
                "nube",
                "case.workflow",
                json!({ "id": case_id, "workflow": "actioned", "ts": DAY + 1 }),
            ),
        }
        .await
        .unwrap_or_else(|e| panic!("{action}: {e:?}"));
        resolve(&node, &p, &id, 2 * DAY).await;
        worked.push((action, case_id));
    }
    let untouched = raise(&node, &p, "c-untouched", DAY).await;
    let untouched_case = case_of(&node, &p, "nube", &untouched).await;
    resolve(&node, &p, &untouched, 2 * DAY).await;

    assert_eq!(
        pass(&node, 3 * DAY).await,
        1,
        "only the untouched case closes"
    );
    assert_eq!(
        case(&node, &p, &untouched_case).await["resolution"],
        "self_cleared"
    );
    for (action, case_id) in worked {
        let c = case(&node, &p, &case_id).await;
        assert_eq!(
            c["closed"], false,
            "a case with a {action} must stay open: {c}"
        );
    }
}

/// **A grouped case waits for EVERY member.** The verdict case holds the root, two symptoms and the
/// verdict record; with three of four resolved it stays open, with the fourth resolved it closes.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_grouped_case_waits_for_every_member() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", &caps());
    install_policy(&node, &p, true).await;

    let root = raise(&node, &p, "flatline", 1).await;
    let sym_a = raise(&node, &p, "valve-hunting", 2).await;
    let sym_b = raise(&node, &p, "temp-drift", 3).await;
    let verdict = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        verdict_input("verdict-1", "flatline", &["valve-hunting", "temp-drift"], 4),
    )
    .await
    .expect("verdict ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let case_id = case_of(&node, &p, "nube", &root).await;
    assert_eq!(
        member_ids(&node, &p, "nube", &case_id).await.len(),
        4,
        "one verdict case over four"
    );

    for id in [&root, &sym_a, &sym_b] {
        resolve(&node, &p, id, DAY).await;
    }
    assert_eq!(
        pass(&node, 2 * DAY).await,
        0,
        "the verdict record is still open, so is the case"
    );
    assert_eq!(case(&node, &p, &case_id).await["closed"], false);

    resolve(&node, &p, &verdict, 3 * DAY).await;
    assert_eq!(pass(&node, 4 * DAY).await, 1);
    assert_eq!(
        case(&node, &p, &case_id).await["resolution"],
        "self_cleared"
    );
}

/// **A problem that comes back is a new episode.** After a self-clear the same dedup key fires again:
/// the insight reopens and the grouping opens a NEW case — `self_cleared` is not held down.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_problem_that_returns_opens_a_new_case() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", &caps());
    install_policy(&node, &p, true).await;

    let id = raise(&node, &p, "flat-5", DAY).await;
    let first = case_of(&node, &p, "nube", &id).await;
    resolve(&node, &p, &id, 2 * DAY).await;
    assert_eq!(pass(&node, 3 * DAY).await, 1);

    raise(&node, &p, "flat-5", 4 * DAY).await;
    let second = case_of(&node, &p, "nube", &id).await;
    assert_ne!(
        second, first,
        "a returning problem must not reopen the self-cleared case"
    );
    assert_eq!(case(&node, &p, &second).await["workflow"], "to_action");
    assert_eq!(
        case(&node, &p, &first).await["resolution"],
        "self_cleared",
        "history kept"
    );
}
