//! **Assignee notification** — the triage plane's missing arm
//! (`docs/scope/insights/insight-assignee-notify-scope.md`), over a REAL booted `Node`. Real store,
//! real bus, real caps, real subscriptions, real seeded memberships/teams, and real delivered inbox
//! Items read back through the real `inbox.list` verb. NO mocks (CLAUDE §9).
//!
//! Two capabilities under test, deliberately separate because they fire at different times:
//!   **A.** `SubFilter.assignee` as a raise-time AND axis ("a finding my crew owns fired again").
//!   **B.** assignment-time notification ("this just became yours") — which **bypasses the ladder**.
//!
//! Mandatory categories: capability-deny (a denied assign notifies nobody; a revoked channel grant
//! flips the sub dormant instead of posting) + workspace-isolation (a ws-B sub never hears a ws-A
//! assignment, and `"me"` resolves teams only within the sub's own workspace).

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
const RESOLVE: &str = "mcp:insight.resolve:call";
const SUB_CREATE: &str = "mcp:insight.sub.create:call";
const SUB_MUTE: &str = "mcp:insight.sub.mute:call";
const POLICY_GET: &str = "mcp:insight.policy.get:call";
const CHAN_PUB: &str = "bus:chan/*:pub";
const INBOX_LIST: &str = "mcp:inbox.list:call";

fn caps() -> Vec<&'static str> {
    vec![
        RAISE, GET, LIST, ASSIGN, RESOLVE, SUB_CREATE, SUB_MUTE, POLICY_GET, CHAN_PUB, INBOX_LIST,
    ]
}

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

/// Real roster: test + priya are members; `team:mechanical` exists with priya on it.
async fn seed_roster(node: &Arc<Node>, ws: &str) {
    for sub in ["user:test", "user:priya", "user:sam"] {
        membership_add_raw(&node.store, ws, sub, 1).await.unwrap();
    }
    team_create(&node.store, ws, "team:mechanical", "Mechanical crew")
        .await
        .unwrap();
    lb_assets::relate(&node.store, ws, MEMBER, "team:mechanical", "user:priya")
        .await
        .unwrap();
}

fn raise_input(dedup_key: &str, severity: &str, ts: u64) -> Value {
    json!({
        "dedup_key": dedup_key,
        "severity": severity,
        "title": format!("finding {dedup_key}"),
        "origin": { "kind": "rule", "ref": "rule:r1" },
        "ts": ts,
    })
}

async fn seed_insight(node: &Arc<Node>, p: &Principal, ws: &str, key: &str, sev: &str) -> String {
    let out = call(node, p, ws, "insight.raise", raise_input(key, sev, 1000))
        .await
        .expect("raise ok");
    out["id"].as_str().unwrap().to_string()
}

/// Create a subscription into `channel` with `filter`.
async fn sub(node: &Arc<Node>, p: &Principal, ws: &str, channel: &str, filter: Value) -> String {
    let out = call(
        node,
        p,
        ws,
        "insight.sub.create",
        json!({ "sink": { "kind": "channel", "channel": channel }, "filter": filter, "now": 1 }),
    )
    .await
    .expect("sub created");
    out["id"].as_str().unwrap().to_string()
}

/// The delivered Items in a channel — the real record path.
async fn inbox(node: &Arc<Node>, p: &Principal, ws: &str, channel: &str) -> Vec<lb_inbox::Item> {
    lb_host::list_inbox(&node.store, p, ws, channel)
        .await
        .expect("inbox readable")
}

/// How many of a channel's Items are ASSIGNMENT notifications rather than raise-time ladder
/// deliveries. Needed wherever one subscription receives both kinds — counting the whole inbox there
/// would let a ladder post masquerade as the assignment post under test.
fn assign_posts(items: Vec<lb_inbox::Item>) -> usize {
    items
        .into_iter()
        .filter(|i| i.body.contains("assigned to"))
        .count()
}

// --- MANDATORY: capability deny ----------------------------------------------------------------

/// A caller denied `mcp:insight.assign:call` assigns nothing — so it notifies nobody. The deny
/// precedes the write, so there is no assignment for a notification to be about.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_denied_assign_notifies_nobody() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let test = principal("user:test", "nube", &caps());
    let id = seed_insight(&node, &test, "nube", "k1", "warning").await;
    sub(
        &node,
        &test,
        "nube",
        "ops",
        json!({ "assignee": "user:priya" }),
    )
    .await;

    // A reader with no assign cap.
    let reader = principal("user:sam", "nube", &[GET, LIST, INBOX_LIST, CHAN_PUB]);
    assert!(matches!(
        call(
            &node,
            &reader,
            "nube",
            "insight.assign",
            json!({ "id": id, "assignee": "user:priya" })
        )
        .await,
        Err(ToolError::Denied)
    ));

    assert_eq!(
        inbox(&node, &test, "nube", "ops").await.len(),
        0,
        "a denied assign must produce no notification"
    );
}

/// A sub whose owner lost `bus:chan/{channel}:pub` **flips dormant** and posts nothing — the
/// fire-time re-check, on the assignment path exactly as on the raise path. Never a silent stop.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_sub_whose_channel_grant_was_revoked_goes_dormant_instead_of_posting() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let test = principal("user:test", "nube", &caps());
    let id = seed_insight(&node, &test, "nube", "k1", "warning").await;

    // Sam owns a sub created WITH pub rights…
    let sam_full = principal("user:sam", "nube", &caps());
    let sub_id = sub(
        &node,
        &sam_full,
        "nube",
        "sam-ops",
        json!({ "assignee": "user:priya" }),
    )
    .await;
    // …but the stored principal snapshot is what fires. Rewrite it to a caps list without the
    // channel pub grant — a real revoke, through the real sub record.
    let mut row: lb_insights::Subscription = serde_json::from_value(
        lb_store::read(&node.store, "nube", lb_insights::SUB_TABLE, &sub_id)
            .await
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    row.principal = json!(["mcp:inbox.list:call"]); // no bus:chan/*:pub
    lb_store::write(
        &node.store,
        "nube",
        lb_insights::SUB_TABLE,
        &sub_id,
        &serde_json::to_value(&row).unwrap(),
    )
    .await
    .unwrap();

    call(
        &node,
        &test,
        "nube",
        "insight.assign",
        json!({ "id": id, "assignee": "user:priya", "ts": 5000 }),
    )
    .await
    .expect("assign ok");

    assert_eq!(
        inbox(&node, &sam_full, "nube", "sam-ops").await.len(),
        0,
        "a revoked grant posts nothing"
    );
    let after: lb_insights::Subscription = serde_json::from_value(
        lb_store::read(&node.store, "nube", lb_insights::SUB_TABLE, &sub_id)
            .await
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert!(
        after.dormant_reason.is_some(),
        "the sub flipped dormant rather than silently stopping"
    );
}

// --- MANDATORY: workspace isolation ------------------------------------------------------------

/// A ws-B subscription never hears a ws-A assignment, and `"me"` resolves the owner's teams only
/// within the sub's OWN workspace — a same-named team elsewhere must not widen the match.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_b_sub_never_hears_a_ws_a_assignment() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "ws-a").await;
    // ws-B has the same team NAME but priya is NOT on it there.
    membership_add_raw(&node.store, "ws-b", "user:priya", 1)
        .await
        .unwrap();
    team_create(&node.store, "ws-b", "team:mechanical", "Mechanical")
        .await
        .unwrap();

    let a = principal("user:test", "ws-a", &caps());
    let b_priya = principal("user:priya", "ws-b", &caps());

    // Priya subscribes in ws-B to her queue.
    sub(
        &node,
        &b_priya,
        "ws-b",
        "b-ops",
        json!({ "assignee": "me" }),
    )
    .await;

    // A ws-A assignment to the ws-A crew.
    let a_id = seed_insight(&node, &a, "ws-a", "k1", "warning").await;
    call(
        &node,
        &a,
        "ws-a",
        "insight.assign",
        json!({ "id": a_id, "assignee": "team:mechanical", "ts": 5000 }),
    )
    .await
    .expect("assign ok in ws-a");

    assert_eq!(
        inbox(&node, &b_priya, "ws-b", "b-ops").await.len(),
        0,
        "a ws-B subscription hears nothing about a ws-A assignment, even for a same-named team"
    );
}

// --- §1 opt-in only (the upgrade-safety test) --------------------------------------------------

/// A sub WITHOUT an `assignee` axis receives nothing on assignment; one WITH it receives exactly
/// one. This is what makes the feature strictly additive — no existing subscription in any
/// workspace changes behaviour on upgrade.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn only_subs_that_filter_on_assignee_hear_an_assignment() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let test = principal("user:test", "nube", &caps());
    let id = seed_insight(&node, &test, "nube", "k1", "warning").await;

    // The pre-existing shape: "everything in this workspace" — must stay a FINDINGS feed.
    sub(&node, &test, "nube", "everything", json!({})).await;
    // A tag sub — also no assignee axis.
    sub(
        &node,
        &test,
        "nube",
        "tagged",
        json!({ "tags": { "kind": "x" } }),
    )
    .await;
    // The opt-in.
    sub(
        &node,
        &test,
        "nube",
        "queue",
        json!({ "assignee": "user:priya" }),
    )
    .await;

    call(
        &node,
        &test,
        "nube",
        "insight.assign",
        json!({ "id": id, "assignee": "user:priya", "ts": 5000 }),
    )
    .await
    .expect("assign ok");

    assert_eq!(
        inbox(&node, &test, "nube", "everything").await.len(),
        0,
        "a catch-all FINDINGS sub must not become an assignment feed on upgrade"
    );
    assert_eq!(inbox(&node, &test, "nube", "tagged").await.len(), 0);
    assert_eq!(
        inbox(&node, &test, "nube", "queue").await.len(),
        1,
        "the sub that opted in got exactly one"
    );
}

// --- §2 bulk coalescing ------------------------------------------------------------------------

/// One human gesture ⇒ ONE notification naming the count, not one per insight. And the count is what
/// matched this sub's FULL filter, not the number of ids passed.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn bulk_assign_coalesces_to_one_delivery_naming_the_matched_count() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let test = principal("user:test", "nube", &caps());

    // 5 warnings + 3 infos. The sub floors at `warning`, so only 5 should be counted.
    let mut ids = Vec::new();
    for i in 0..5 {
        ids.push(seed_insight(&node, &test, "nube", &format!("w{i}"), "warning").await);
    }
    for i in 0..3 {
        ids.push(seed_insight(&node, &test, "nube", &format!("i{i}"), "info").await);
    }
    sub(
        &node,
        &test,
        "nube",
        "queue",
        json!({ "assignee": "user:priya", "severity_min": "warning" }),
    )
    .await;

    call(
        &node,
        &test,
        "nube",
        "insight.assign",
        json!({ "ids": ids, "assignee": "user:priya", "ts": 5000 }),
    )
    .await
    .expect("bulk assign ok");

    let items = inbox(&node, &test, "nube", "queue").await;
    assert_eq!(
        items.len(),
        1,
        "8 assignments in one gesture = ONE notification, not 8: {items:?}"
    );
    assert!(
        items[0].body.contains('5'),
        "it names the count that matched the sub's OWN filter (5 warnings, not 8 ids): {}",
        items[0].body
    );
}

/// A single assign names the finding (that is the useful thing for one).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_single_assign_names_the_finding() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let test = principal("user:test", "nube", &caps());
    let id = seed_insight(&node, &test, "nube", "chullora-1", "warning").await;
    sub(
        &node,
        &test,
        "nube",
        "queue",
        json!({ "assignee": "user:priya" }),
    )
    .await;

    call(
        &node,
        &test,
        "nube",
        "insight.assign",
        json!({ "id": id, "assignee": "user:priya", "ts": 5000 }),
    )
    .await
    .expect("assign ok");

    let items = inbox(&node, &test, "nube", "queue").await;
    assert_eq!(items.len(), 1);
    assert!(
        items[0].body.contains("chullora-1"),
        "the single case names the finding: {}",
        items[0].body
    );
}

// --- §3 `"me"` resolves owner + teams ----------------------------------------------------------

/// A finding assigned to `team:mechanical` notifies a sub owned by a CREW MEMBER filtering
/// `assignee: "me"` — and not an identical sub owned by a non-member. The case a naive
/// owner-equality check silently drops.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn me_resolves_to_the_sub_owner_and_their_teams() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let test = principal("user:test", "nube", &caps());
    let priya = principal("user:priya", "nube", &caps()); // on team:mechanical
    let sam = principal("user:sam", "nube", &caps()); // NOT on the crew

    sub(
        &node,
        &priya,
        "nube",
        "priya-q",
        json!({ "assignee": "me" }),
    )
    .await;
    sub(&node, &sam, "nube", "sam-q", json!({ "assignee": "me" })).await;

    let id = seed_insight(&node, &test, "nube", "k1", "warning").await;
    call(
        &node,
        &test,
        "nube",
        "insight.assign",
        json!({ "id": id, "assignee": "team:mechanical", "ts": 5000 }),
    )
    .await
    .expect("assign ok");

    assert_eq!(
        inbox(&node, &priya, "nube", "priya-q").await.len(),
        1,
        "a TEAM assignment reaches the crew member's 'me' queue — the whole point of team subjects"
    );
    assert_eq!(
        inbox(&node, &sam, "nube", "sam-q").await.len(),
        0,
        "and not a non-member's identical sub"
    );
}

// --- §4 self-assign is silent ------------------------------------------------------------------

/// "I'll take this" notifies nobody; assigning to a team the actor is ON still does.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn self_assignment_is_silent_but_assigning_to_your_own_team_is_not() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let priya = principal("user:priya", "nube", &caps());
    sub(&node, &priya, "nube", "q", json!({ "assignee": "me" })).await;

    // Priya assigns to HERSELF — her own action, no news.
    let a = seed_insight(&node, &priya, "nube", "k1", "warning").await;
    call(
        &node,
        &priya,
        "nube",
        "insight.assign",
        json!({ "id": a, "assignee": "user:priya", "ts": 5000 }),
    )
    .await
    .expect("self assign ok");
    assert_eq!(
        inbox(&node, &priya, "nube", "q").await.len(),
        0,
        "telling someone about their own action is the noise that makes people mute a channel"
    );

    // Priya assigns to her CREW — the assignee is the queue, not her; the crew needs to know.
    let b = seed_insight(&node, &priya, "nube", "k2", "warning").await;
    call(
        &node,
        &priya,
        "nube",
        "insight.assign",
        json!({ "id": b, "assignee": "team:mechanical", "ts": 5001 }),
    )
    .await
    .expect("team assign ok");
    assert_eq!(
        inbox(&node, &priya, "nube", "q").await.len(),
        1,
        "assigning to a team you are on still notifies the queue"
    );
}

// --- §5 THE design decision: the ladder is bypassed --------------------------------------------

/// **The assertion that pins the whole design.** An assignment must deliver even when the ladder
/// state for **that same `(sub, dedup_key)`** is deep in cooldown.
///
/// Constructing this properly is the whole point, and it is easy to get wrong: the collision exists
/// only if the SAME subscription is both the findings feed and the assignee queue for the SAME key.
/// (An earlier version of this test used two separate subs, so the flapping heated a *different*
/// ladder key and the assertion held no matter what the code did — it passed a revert-check that
/// should have failed.) So: assign first, so the assignee-filtered sub also matches on the raise
/// path; flap the key, heating THAT sub's ladder state for THAT key; then change the owner again.
///
/// If assignment were routed through `apply_intents`, that last step would be swallowed by the
/// cooldown — the one message that must never be suppressed by a finding's noise.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_assignment_delivers_even_when_that_subs_ladder_is_in_cooldown_for_that_key() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let test = principal("user:test", "nube", &caps());

    // ONE sub: an assignee queue that will ALSO match on the raise path once the finding is owned.
    sub(
        &node,
        &test,
        "nube",
        "q",
        json!({ "assignee": "user:priya" }),
    )
    .await;

    let id = seed_insight(&node, &test, "nube", "flapper", "warning").await;
    call(
        &node,
        &test,
        "nube",
        "insight.assign",
        json!({ "id": id, "assignee": "user:priya", "ts": 1000 }),
    )
    .await
    .expect("first assign");

    // Flap the key. Every raise now matches this sub (the finding is priya's), so ITS ladder state
    // for THIS key heats up and the L0 cooldown engages.
    for ts in 1..9u64 {
        call(
            &node,
            &test,
            "nube",
            "insight.raise",
            raise_input("flapper", "warning", ts * 1000),
        )
        .await
        .expect("raise");
    }
    let total_before = inbox(&node, &test, "nube", "q").await.len();
    assert!(
        total_before < 9,
        "precondition: the ladder IS throttling this key for THIS sub ({total_before} posts for \
         8 raises + 1 assign)"
    );
    assert_eq!(
        assign_posts(inbox(&node, &test, "nube", "q").await),
        1,
        "one assignment post so far"
    );

    // A genuine change of owner while that cooldown is hot.
    call(
        &node,
        &test,
        "nube",
        "insight.assign",
        json!({ "id": id, "assignee": null, "ts": 9000 }),
    )
    .await
    .expect("un-assign");
    call(
        &node,
        &test,
        "nube",
        "insight.assign",
        json!({ "id": id, "assignee": "user:priya", "ts": 9001 }),
    )
    .await
    .expect("re-assign");

    assert_eq!(
        assign_posts(inbox(&node, &test, "nube", "q").await),
        2,
        "the re-assignment delivered THROUGH the flapping key's cooldown on the very same sub — \
         assignment is a one-shot human act, not a firing, and must not share a finding's \
         anti-spam state"
    );
}

/// An idempotent re-assign (same owner) writes nothing and must announce nothing — a double-click or
/// a retried bulk call is not a second event.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn re_assigning_the_same_owner_does_not_re_notify() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let test = principal("user:test", "nube", &caps());
    let id = seed_insight(&node, &test, "nube", "k1", "warning").await;
    sub(
        &node,
        &test,
        "nube",
        "q",
        json!({ "assignee": "user:priya" }),
    )
    .await;

    for ts in [5000u64, 5001, 5002] {
        call(
            &node,
            &test,
            "nube",
            "insight.assign",
            json!({ "id": id, "assignee": "user:priya", "ts": ts }),
        )
        .await
        .expect("assign ok");
    }

    assert_eq!(
        inbox(&node, &test, "nube", "q").await.len(),
        1,
        "three identical assigns = one notification (only a real change of owner is an event)"
    );
}

// --- §6 muted / kill-switch ---------------------------------------------------------------------

/// A muted sub receives nothing (the same rule the raise path holds).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_muted_sub_receives_no_assignment_notification() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let test = principal("user:test", "nube", &caps());
    let id = seed_insight(&node, &test, "nube", "k1", "warning").await;
    let sub_id = sub(
        &node,
        &test,
        "nube",
        "q",
        json!({ "assignee": "user:priya" }),
    )
    .await;

    call(
        &node,
        &test,
        "nube",
        "insight.sub.mute",
        json!({ "id": sub_id, "muted": true }),
    )
    .await
    .expect("mute ok");

    call(
        &node,
        &test,
        "nube",
        "insight.assign",
        json!({ "id": id, "assignee": "user:priya", "ts": 5000 }),
    )
    .await
    .expect("assign ok");

    assert_eq!(
        inbox(&node, &test, "nube", "q").await.len(),
        0,
        "muted = silent"
    );
}

/// The per-member kill switch silences assignment notifications for that owner too.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_member_kill_switch_silences_assignment_notifications() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let test = principal("user:test", "nube", &caps());
    let priya = principal("user:priya", "nube", &caps());
    let id = seed_insight(&node, &test, "nube", "k1", "warning").await;
    sub(&node, &priya, "nube", "q", json!({ "assignee": "me" })).await;

    // Priya turns the whole insight-notification system off for herself.
    let mut prefs = lb_prefs::get_user_prefs(&node.store, "nube", "user:priya")
        .await
        .unwrap()
        .unwrap_or_default();
    prefs.insight_notifications = Some(false);
    lb_prefs::set_user_prefs(&node.store, "nube", "user:priya", &prefs, &[])
        .await
        .unwrap();

    call(
        &node,
        &test,
        "nube",
        "insight.assign",
        json!({ "id": id, "assignee": "user:priya", "ts": 5000 }),
    )
    .await
    .expect("assign ok");

    assert_eq!(
        inbox(&node, &priya, "nube", "q").await.len(),
        0,
        "the kill switch covers assignment notifications, not just firings"
    );
}

// --- §7 un-assign notifies nobody --------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn un_assignment_notifies_nobody() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let test = principal("user:test", "nube", &caps());
    let id = seed_insight(&node, &test, "nube", "k1", "warning").await;
    sub(
        &node,
        &test,
        "nube",
        "q",
        json!({ "assignee": "user:priya" }),
    )
    .await;

    call(
        &node,
        &test,
        "nube",
        "insight.assign",
        json!({ "id": id, "assignee": "user:priya", "ts": 5000 }),
    )
    .await
    .expect("assign ok");
    assert_eq!(inbox(&node, &test, "nube", "q").await.len(), 1);

    // Clearing it is not news worth a channel post.
    call(
        &node,
        &test,
        "nube",
        "insight.assign",
        json!({ "id": id, "assignee": null, "ts": 6000 }),
    )
    .await
    .expect("un-assign ok");
    assert_eq!(
        inbox(&node, &test, "nube", "q").await.len(),
        1,
        "un-assignment added nothing"
    );
}

// --- §8 capability A: the raise-time assignee axis ---------------------------------------------

/// The other half: once a finding is owned, a raise-time `assignee` filter matches it — so a re-fire
/// of "something my crew owns" flows through the NORMAL ladder. An unassigned finding never matches
/// a filter that names an assignee.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_raise_time_assignee_axis_matches_only_owned_findings() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let test = principal("user:test", "nube", &caps());
    let priya = principal("user:priya", "nube", &caps());

    sub(&node, &priya, "nube", "crew", json!({ "assignee": "me" })).await;

    // An UNASSIGNED finding fires — the assignee axis must not match "anything, including
    // what nobody owns".
    call(
        &node,
        &test,
        "nube",
        "insight.raise",
        raise_input("unowned", "warning", 1000),
    )
    .await
    .expect("raise");
    assert_eq!(
        inbox(&node, &priya, "nube", "crew").await.len(),
        0,
        "an unassigned finding never matches a filter that names an assignee"
    );

    // Assign one to the crew, then let it fire again.
    let id = seed_insight(&node, &test, "nube", "owned", "warning").await;
    call(
        &node,
        &test,
        "nube",
        "insight.assign",
        json!({ "id": id, "assignee": "team:mechanical", "ts": 2000 }),
    )
    .await
    .expect("assign ok");
    let after_assign = inbox(&node, &priya, "nube", "crew").await.len();

    call(
        &node,
        &test,
        "nube",
        "insight.raise",
        raise_input("owned", "critical", 3000),
    )
    .await
    .expect("re-raise");

    assert!(
        inbox(&node, &priya, "nube", "crew").await.len() > after_assign,
        "a re-fire of a crew-owned finding reaches the crew's subscription through the ladder"
    );
}

/// The assignee axis ANDs with the others, exactly like every other filter dimension.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_assignee_axis_ands_with_the_other_filter_axes() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let test = principal("user:test", "nube", &caps());

    // Owned by priya, but only CRITICAL is wanted.
    sub(
        &node,
        &test,
        "nube",
        "q",
        json!({ "assignee": "user:priya", "severity_min": "critical" }),
    )
    .await;

    let warn = seed_insight(&node, &test, "nube", "warn", "warning").await;
    call(
        &node,
        &test,
        "nube",
        "insight.assign",
        json!({ "id": warn, "assignee": "user:priya", "ts": 5000 }),
    )
    .await
    .expect("assign ok");
    assert_eq!(
        inbox(&node, &test, "nube", "q").await.len(),
        0,
        "the severity floor still applies to an assignment notification"
    );

    let crit = seed_insight(&node, &test, "nube", "crit", "critical").await;
    call(
        &node,
        &test,
        "nube",
        "insight.assign",
        json!({ "id": crit, "assignee": "user:priya", "ts": 5001 }),
    )
    .await
    .expect("assign ok");
    assert_eq!(
        inbox(&node, &test, "nube", "q").await.len(),
        1,
        "and a critical one gets through"
    );
}

/// An assignment to someone the sub did NOT ask about reaches nobody.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_assignment_to_another_subject_reaches_nobody() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let test = principal("user:test", "nube", &caps());
    let id = seed_insight(&node, &test, "nube", "k1", "warning").await;
    sub(
        &node,
        &test,
        "nube",
        "q",
        json!({ "assignee": "user:priya" }),
    )
    .await;

    call(
        &node,
        &test,
        "nube",
        "insight.assign",
        json!({ "id": id, "assignee": "user:sam", "ts": 5000 }),
    )
    .await
    .expect("assign ok");

    assert_eq!(
        inbox(&node, &test, "nube", "q").await.len(),
        0,
        "a queue subscribed to priya hears nothing about sam's work"
    );
}
