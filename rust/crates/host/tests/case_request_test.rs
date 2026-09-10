//! The **contractor round trip** — `party.*`, `case.request.*`, the link email, the nudge ladder
//! and the token principal, over a REAL booted `Node` (`docs/scope/insights/case-plane-scope.md`,
//! wave 2).
//!
//! Real store (`mem://`), real bus, real caps, the real `call_tool` MCP bridge, the real outbox
//! relay and the real reminder reactor. The ONE permitted external is the email provider, behind
//! the `EmailProvider` trait — and both shipped non-transport impls are used deliberately:
//! `RecordingEmailProvider` to read what was actually mailed (which is how these tests get a
//! working token: the raw string exists ONLY in the link, exactly as production says it does), and
//! `LoggingEmailProvider` to prove the thing resolved decision 7 exists for — that a node with no
//! mailer yields `delivery: logged` and NEVER `sent`.
//!
//! Mandatory categories, per new verb: **capability-deny** and **workspace-isolation**. Plus a
//! POSITIVE gate test per verb, because a missing `tool_gate.rs` arm is `Denied`, not `NotFound`.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_authz::membership_add_raw;
use lb_host::{
    call_tool, relay_outbox, EmailTarget, LoggingEmailProvider, Node, RecordingEmailProvider,
};
use lb_mcp::ToolError;
use serde_json::{json, Value};

const RAISE: &str = "mcp:insight.raise:call";
const I_GET: &str = "mcp:insight.get:call";
const GET: &str = "mcp:case.get:call";
const OPEN: &str = "mcp:case.open:call";
const WORKFLOW: &str = "mcp:case.workflow:call";
const SEND: &str = "mcp:case.request.send:call";
const P_UPSERT: &str = "mcp:party.upsert:call";
const P_LIST: &str = "mcp:party.list:call";

/// A fully-empowered operator: every cap this suite needs to SEED with.
const ALL: &[&str] = &[RAISE, I_GET, GET, OPEN, WORKFLOW, SEND, P_UPSERT, P_LIST];

/// One hour in the epoch-ms the case plane stores.
const HOUR_MS: u64 = 60 * 60 * 1000;

/// A logical "now" well inside the epoch-ms band, so the host's `normalize_ts` passes it through.
const T0: u64 = 1_800_000_000_000;

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
    Ok(serde_json::from_str(&out).unwrap_or(Value::Null))
}

/// Grant `cap` to `user` in the DURABLE grant store — what the reminder's fire-time re-resolve
/// reads. The sub is parsed exactly as `grants.assign` parses it, so the row lands under the bare
/// handle the store keys on (wrapping it verbatim hides a dead fire path).
async fn grant(store: &lb_store::Store, ws: &str, user: &str, cap: &str) {
    let s = lb_authz::Subject::parse(user).expect("a subject like `user:test`");
    lb_authz::grant_assign(store, ws, &s, cap).await.unwrap();
}

/// Raise one insight; the inline grouping reactor opens its case. Returns the case id.
async fn seed_case(node: &Arc<Node>, p: &Principal, ws: &str, key: &str) -> String {
    let raised = call(
        node,
        p,
        ws,
        "insight.raise",
        json!({
            "dedup_key": key,
            "severity": "warning",
            "title": format!("finding {key}"),
            "origin": { "kind": "rule", "ref": "rule:probe" },
            "ts": T0,
        }),
    )
    .await
    .expect("raise ok");
    let insight = raised["id"].as_str().unwrap().to_string();
    let got = call(node, p, ws, "insight.get", json!({ "id": insight }))
        .await
        .expect("get ok");
    got["case_id"]
        .as_str()
        .unwrap_or_else(|| panic!("no case echo: {got}"))
        .to_string()
}

/// Put one party in the roster. `window_h` of 0 means "no opinion" (the policy/default decides).
async fn seed_party(node: &Arc<Node>, p: &Principal, ws: &str, id: &str, window_h: u32) {
    call(
        node,
        p,
        ws,
        "party.upsert",
        json!({
            "id": id,
            "kind": "contractor",
            "name": format!("{id} services"),
            "contact": { "email": format!("{id}@example.com") },
            "sites": ["site-a"],
            "trades": ["mechanical"],
            "default_ask_window_h": window_h,
        }),
    )
    .await
    .expect("party.upsert ok");
}

async fn seed_roster(node: &Arc<Node>, ws: &str) {
    membership_add_raw(&node.store, ws, "user:test", 1)
        .await
        .expect("test joins");
}

/// Drain the outbox through the REAL relay and the REAL email target, recording what was mailed.
async fn drain_recording(node: &Arc<Node>, ws: &str) -> Arc<RecordingEmailProvider> {
    let provider = Arc::new(RecordingEmailProvider::default());
    let target = EmailTarget::new(Box::new(provider.clone()), node.store.clone());
    relay_outbox(&node.store, ws, &target, T0 / 1000)
        .await
        .expect("a relay pass");
    provider
}

/// Drain the outbox through the LOGGING provider — a node with no mailer configured.
async fn drain_logging(node: &Arc<Node>, ws: &str) {
    let target = EmailTarget::new(Box::new(LoggingEmailProvider), node.store.clone());
    relay_outbox(&node.store, ws, &target, T0 / 1000)
        .await
        .expect("a relay pass");
}

/// The raw token out of the mail body — the ONLY place it exists (the row keeps a hash). This is
/// also the assertion that the link email actually carries a usable link.
fn token_from(body: &str) -> String {
    let at = body
        .find("/r/")
        .unwrap_or_else(|| panic!("no link in the mail: {body}"));
    body[at + 3..]
        .split_whitespace()
        .next()
        .expect("a token after /r/")
        .trim_end_matches(['.', ',', '<', '"'])
        .to_string()
}

/// Send an ask and return `(request_id, raw_token)` — the send, one relay pass, and the token read
/// back out of what was actually mailed.
async fn send_ask(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    case_id: &str,
    party: &str,
) -> (String, String) {
    let request = call(
        node,
        p,
        ws,
        "case.request.send",
        json!({ "case_id": case_id, "party_id": party, "ask": "quote", "ts": T0 }),
    )
    .await
    .expect("send ok");
    let id = request["id"].as_str().unwrap().to_string();
    let provider = drain_recording(node, ws).await;
    let sends = provider.sends();
    assert_eq!(sends.len(), 1, "one link email per ask");
    (id, token_from(&sends[0].body))
}

/// The case's history, newest-first page, as raw event rows.
async fn events_of(node: &Arc<Node>, p: &Principal, ws: &str, case_id: &str) -> Vec<Value> {
    let page = call(
        node,
        p,
        ws,
        "case.events",
        json!({ "case_id": case_id, "limit": 100 }),
    )
    .await
    .expect("events ok");
    page["items"]
        .as_array()
        .cloned()
        .unwrap_or_else(|| panic!("the event page has no `items`: {page}"))
}

fn count_kind(events: &[Value], kind: &str) -> usize {
    events
        .iter()
        .filter(|e| e["kind"].as_str() == Some(kind))
        .count()
}

// --- MANDATORY: capability deny -----------------------------------------------------------------

/// Every new verb refuses a principal that does not hold its capability. One test over the whole
/// wave-2 surface, so a verb added later without a gate fails here rather than shipping open.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn every_request_verb_denies_a_principal_without_its_cap() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &full, ws, "deny-probe").await;
    seed_party(&node, &full, ws, "northern", 24).await;

    // A principal with every OTHER case cap and none of the new ones.
    let bare = principal("user:test", ws, &[RAISE, I_GET, GET, OPEN, WORKFLOW]);
    for (tool, input) in [
        (
            "party.upsert",
            json!({ "id": "x", "kind": "contractor", "name": "X" }),
        ),
        ("party.list", json!({})),
        (
            "case.request.send",
            json!({ "case_id": case_id, "party_id": "northern", "ask": "quote" }),
        ),
        ("case.request.withdraw", json!({ "id": "whatever" })),
        (
            "case.request.nudge",
            json!({ "id": "whatever", "stage": "n50" }),
        ),
    ] {
        let err = call(&node, &bare, ws, tool, input)
            .await
            .expect_err(&format!("{tool} must deny"));
        assert!(matches!(err, ToolError::Denied), "{tool}: {err:?}");
    }

    // The two TOKEN verbs are refused even for a fully-empowered member: their caps are in no role
    // bundle, deliberately, and nothing but a presented link mints them.
    for (tool, input) in [
        ("case.request.view", json!({ "id": "whatever" })),
        (
            "case.request.reply",
            json!({ "id": "whatever", "reply": { "kind": "accept" } }),
        ),
    ] {
        let err = call(&node, &full, ws, tool, input)
            .await
            .expect_err(&format!("{tool} must deny a logged-in caller"));
        assert!(matches!(err, ToolError::Denied), "{tool}: {err:?}");
    }
}

// --- MANDATORY: workspace isolation -------------------------------------------------------------

/// A ws-B principal cannot see or touch a ws-A party or ask — and a REAL ws-A id is refused
/// identically to a fictional one, so a probe learns nothing about what exists.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_b_principal_cannot_reach_a_ws_a_party_or_request() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "acme").await;
    let a = principal("user:test", "acme", ALL);
    let case_id = seed_case(&node, &a, "acme", "iso-probe").await;
    seed_party(&node, &a, "acme", "northern", 24).await;
    let (request_id, token) = send_ask(&node, &a, "acme", &case_id, "northern").await;

    // Fully capped — in another workspace.
    let b = principal("user:test", "other", ALL);
    for (tool, input) in [
        ("party.list", json!({})),
        (
            "case.request.send",
            json!({ "case_id": case_id, "party_id": "northern", "ask": "quote" }),
        ),
        ("case.request.withdraw", json!({ "id": request_id })),
    ] {
        let err = call(&node, &b, "acme", tool, input)
            .await
            .expect_err(&format!("{tool} must deny across the wall"));
        assert!(matches!(err, ToolError::Denied), "{tool}: {err:?}");
    }

    // The party roster does not leak either: ws `other` sees an empty roster, not acme's.
    let roster = call(&node, &b, "other", "party.list", json!({}))
        .await
        .expect("own-workspace list is allowed");
    assert_eq!(
        roster.as_array().map(Vec::len),
        Some(0),
        "a ws-B roster must never contain a ws-A party"
    );

    // And the TOKEN is workspace-bound: presenting acme's token against `other` resolves nothing.
    let wrong_ws = lb_host::case_request_authenticate(&node.store, "other", &token, T0).await;
    assert!(
        wrong_ws.is_err(),
        "a token must not resolve in another workspace"
    );
}

// --- MANDATORY: the positive gate test per verb --------------------------------------------------

/// Each aliased verb resolves to a cap a role actually holds, and each namesake cap exists in a
/// shipped bundle. A missing `tool_gate.rs` arm is `Denied`, not `NotFound`, so only this catches it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_new_verb_resolves_to_a_cap_a_role_actually_holds() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &full, ws, "gate-probe").await;
    seed_party(&node, &full, ws, "northern", 24).await;

    // A token holding ONLY `case.request.send` reaches send, withdraw AND nudge.
    let sender = principal("user:test", ws, &[SEND]);
    let request = call(
        &node,
        &sender,
        ws,
        "case.request.send",
        json!({ "case_id": case_id, "party_id": "northern", "ask": "attend", "ts": T0 }),
    )
    .await
    .expect("case.request.send gates on its own cap");
    let id = request["id"].as_str().unwrap().to_string();
    call(
        &node,
        &sender,
        ws,
        "case.request.nudge",
        json!({ "id": id, "stage": "n50", "ts": T0 + HOUR_MS }),
    )
    .await
    .expect("case.request.nudge must resolve through the `case.request.send` alias");
    call(
        &node,
        &sender,
        ws,
        "case.request.withdraw",
        json!({ "id": id, "ts": T0 + 2 * HOUR_MS }),
    )
    .await
    .expect("case.request.withdraw must resolve through the `case.request.send` alias");

    // A token holding ONLY the case READ cap reaches the drawer's ask list.
    let reader = principal("user:test", ws, &[GET]);
    call(
        &node,
        &reader,
        ws,
        "case.request.list",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("case.request.list must resolve through the `case.get` alias");
}

/// The other half of the trap: the caps the aliases point at must EXIST in a shipped bundle — and
/// the two TOKEN caps must exist in NONE, which is the design and not an oversight.
#[test]
fn the_request_caps_sit_in_the_right_bundles_and_the_token_caps_in_none() {
    let viewer = lb_host::viewer_role_caps();
    let member = lb_host::member_role_caps();
    let admin = lb_host::workspace_admin_role_caps();

    assert!(
        member.iter().any(|c| c == SEND),
        "{SEND} must be a MEMBER cap"
    );
    assert!(
        !viewer.iter().any(|c| c == SEND),
        "a viewer must not be able to email the outside world"
    );
    for cap in [P_UPSERT, P_LIST] {
        assert!(admin.iter().any(|c| c == cap), "{cap} must be an ADMIN cap");
        assert!(
            !member.iter().any(|c| c == cap),
            "{cap} must NOT be a member cap — the roster is other companies' contact details"
        );
    }
    for cap in [lb_host::VIEW_CAP, lb_host::REPLY_CAP] {
        for (name, bundle) in [("viewer", &viewer), ("member", &member), ("admin", &admin)] {
            assert!(
                !bundle.iter().any(|c| c == cap),
                "{cap} must be in NO bundle ({name}) — it is minted onto the token principal only"
            );
        }
    }
}

// --- The send, end to end ------------------------------------------------------------------------

/// One send does all six things: the row, the mail with a working link, the window, the ladder, the
/// case's `waiting_on`, and the `request_sent` event.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_send_mints_a_link_sets_the_window_and_moves_the_ball() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &full, ws, "send-probe").await;
    seed_party(&node, &full, ws, "northern", 10).await;

    let request = call(
        &node,
        &full,
        ws,
        "case.request.send",
        json!({
            "case_id": case_id,
            "party_id": "northern",
            "ask": "quote",
            "brief": {
                "ask_text": "Please price replacing the actuator.",
                "currency": "AUD",
                "asset": "AHU-3",
                "evidence": {
                    "series": [ { "name": "supply temp", "points": [[1.0, 21.5], [2.0, 24.0]] } ],
                    "threshold": 22.0,
                    "unit": "degC"
                }
            },
            "ts": T0,
        }),
    )
    .await
    .expect("send ok");

    // The window came from the PARTY's own 10 hours, and both deadlines are set from it.
    assert_eq!(request["expires_ts"].as_u64(), Some(T0 + 10 * HOUR_MS));
    assert_eq!(request["respond_by"].as_u64(), Some(T0 + 10 * HOUR_MS));
    assert_eq!(request["status"].as_str(), Some("sent"));
    assert_eq!(
        request["delivery"].as_str(),
        Some("queued"),
        "nothing has been attempted yet — `queued`, never `sent`"
    );
    // Only the hash is stored, and the response does not hand back a working link.
    assert_eq!(request["token_hash"].as_str().map(str::len), Some(64));
    assert!(
        !request.to_string().contains("lbr_"),
        "the raw token must never appear in the send reply: {request}"
    );

    // The ball moved, and the history says so.
    let case = call(&node, &full, ws, "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert_eq!(case["waiting_on"].as_str(), Some("contractor"));
    let events = events_of(&node, &full, ws, &case_id).await;
    assert_eq!(count_kind(&events, "request_sent"), 1);

    // The ladder: three one-shot reminders at 50 % / 80 % / breach of the 10-hour window.
    let request_id = request["id"].as_str().unwrap();
    let expected = [
        (T0 + 5 * HOUR_MS) / 1000,
        (T0 + 8 * HOUR_MS) / 1000,
        (T0 + 10 * HOUR_MS) / 1000,
    ];
    for (stage, at) in ["n50", "n80", "breach"].iter().zip(expected.iter()) {
        let reminder =
            lb_reminders::load(&node.store, ws, &format!("case-nudge-{request_id}-{stage}"))
                .await
                .expect("load ok")
                .unwrap_or_else(|| panic!("no {stage} nudge scheduled"));
        assert_eq!(
            reminder.next_attempt_ts, *at,
            "the {stage} rung must be pinned to the exact instant, not the next cron slot"
        );
        assert_eq!(reminder.max_runs, Some(1), "a nudge is a one-shot");
        assert_eq!(
            reminder.principal_sub, "user:test",
            "the ladder fires under the SENDER, so revoking their grant stops it"
        );
    }

    // And the mail itself carries a link the gateway can resolve.
    let provider = drain_recording(&node, ws).await;
    let sends = provider.sends();
    assert_eq!(sends[0].to, "northern@example.com");
    assert!(
        !sends[0].subject.is_empty(),
        "the catalog supplies a subject"
    );
    let token = token_from(&sends[0].body);
    assert!(token.starts_with("lbr_nube."), "{token}");
    lb_host::case_request_authenticate(&node.store, ws, &token, T0 + 1)
        .await
        .expect("the mailed token must actually work");
}

/// The window falls back to the POLICY when the party has no opinion of its own.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_party_with_no_window_falls_back_to_the_policy() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let admin = principal("user:test", ws, &["mcp:policy.sla.set:call"]);
    let case_id = seed_case(&node, &full, ws, "window-probe").await;
    seed_party(&node, &full, ws, "quiet", 0).await;

    call(
        &node,
        &admin,
        ws,
        "policy.sla.set",
        json!({
            "id": "default",
            "name": "workspace default",
            "match": {},
            "respond_h": 4,
            "resolve_h": 48,
            "party_window_h": 6,
            "hold_down_days": 14,
        }),
    )
    .await
    .expect("policy.sla.set ok");

    let request = call(
        &node,
        &full,
        ws,
        "case.request.send",
        json!({ "case_id": case_id, "party_id": "quiet", "ask": "info", "ts": T0 }),
    )
    .await
    .expect("send ok");
    assert_eq!(
        request["expires_ts"].as_u64(),
        Some(T0 + 6 * HOUR_MS),
        "the policy's party_window_h must win when the party states none"
    );
}

// --- Resolved decision 7: delivery mirrors the PROVIDER ------------------------------------------

/// **The load-bearing one.** A dev node's `LoggingEmailProvider` acknowledges every mail it DROPS,
/// so the outbox row reads `delivered`. `delivery` must still read `logged` — never `sent` — or the
/// drawer tells an operator an email went out that did not
/// (`outbox-delivered-is-not-email-sent.md`).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delivery_says_logged_when_the_node_has_no_mailer() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &full, ws, "logged-probe").await;
    seed_party(&node, &full, ws, "northern", 24).await;
    call(
        &node,
        &full,
        ws,
        "case.request.send",
        json!({ "case_id": case_id, "party_id": "northern", "ask": "quote", "ts": T0 }),
    )
    .await
    .expect("send ok");

    // The REAL relay, the REAL email target, the shipped logging provider.
    drain_logging(&node, ws).await;

    let listed = call(
        &node,
        &full,
        ws,
        "case.request.list",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("list ok");
    let row = &listed.as_array().expect("an array")[0];
    assert_eq!(
        row["delivery"].as_str(),
        Some("logged"),
        "a logging provider must never yield `sent`: {row}"
    );
    assert!(
        row.get("token_hash").is_none(),
        "the drawer must never be handed the credential: {row}"
    );
}

/// The same path with a provider that really hands the message on yields `sent` — so the previous
/// test is testing the PROVIDER KIND and not simply a field that is always `logged`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delivery_says_sent_when_a_real_provider_took_the_message() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &full, ws, "sent-probe").await;
    seed_party(&node, &full, ws, "northern", 24).await;
    let (_id, _token) = send_ask(&node, &full, ws, &case_id, "northern").await;

    let listed = call(
        &node,
        &full,
        ws,
        "case.request.list",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("list ok");
    assert_eq!(
        listed.as_array().expect("an array")[0]["delivery"].as_str(),
        Some("sent")
    );
}

// --- The token principal --------------------------------------------------------------------------

/// A token views and replies on ITS OWN request only; any other request, and any other verb, is
/// `Denied`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_token_reaches_its_own_request_and_nothing_else() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_a = seed_case(&node, &full, ws, "token-a").await;
    let case_b = seed_case(&node, &full, ws, "token-b").await;
    seed_party(&node, &full, ws, "northern", 24).await;
    seed_party(&node, &full, ws, "southern", 24).await;

    let (id_a, token_a) = send_ask(&node, &full, ws, &case_a, "northern").await;
    let (id_b, _token_b) = send_ask(&node, &full, ws, &case_b, "southern").await;

    let (party, _row) = lb_host::case_request_authenticate(&node.store, ws, &token_a, T0 + 1)
        .await
        .expect("a live token resolves");
    assert_eq!(party.sub(), "party:northern", "attributed to the PARTY");

    // Its own request: fine.
    let view = call(
        &node,
        &party,
        ws,
        "case.request.view",
        json!({ "id": id_a, "ts": T0 + 1 }),
    )
    .await
    .expect("a token views its own request");
    assert_eq!(view["request_id"].as_str(), Some(id_a.as_str()));
    assert_eq!(view["party_name"].as_str(), Some("northern services"));
    assert!(
        view.get("case_id").is_none() && view.get("token_hash").is_none(),
        "the view must not carry the case id or the credential: {view}"
    );

    // Another party's request: denied, on both verbs.
    for (tool, input) in [
        ("case.request.view", json!({ "id": id_b, "ts": T0 + 1 })),
        (
            "case.request.reply",
            json!({ "id": id_b, "reply": { "kind": "accept" }, "ts": T0 + 1 }),
        ),
    ] {
        let err = call(&node, &party, ws, tool, input)
            .await
            .expect_err(&format!("{tool} on another request must deny"));
        assert!(matches!(err, ToolError::Denied), "{tool}: {err:?}");
    }

    // Every other verb on the node: denied.
    for (tool, input) in [
        ("case.get", json!({ "id": case_a })),
        ("case.list", json!({ "lane": "watching" })),
        ("case.events", json!({ "case_id": case_a })),
        (
            "case.workflow",
            json!({ "id": case_a, "workflow": "actioned" }),
        ),
        (
            "case.request.send",
            json!({ "case_id": case_a, "party_id": "southern", "ask": "quote" }),
        ),
        ("party.list", json!({})),
        (
            "insight.raise",
            json!({ "dedup_key": "x", "severity": "info", "title": "x" }),
        ),
    ] {
        let err = call(&node, &party, ws, tool, input)
            .await
            .expect_err(&format!("{tool} must deny a token principal"));
        assert!(matches!(err, ToolError::Denied), "{tool}: {err:?}");
    }
}

/// A withdrawn ask, and an expired one, are both `Gone` — and the caller cannot tell which.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_withdrawn_or_expired_token_is_gone_and_never_says_which() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_a = seed_case(&node, &full, ws, "gone-a").await;
    let case_b = seed_case(&node, &full, ws, "gone-b").await;
    seed_party(&node, &full, ws, "northern", 2).await;
    seed_party(&node, &full, ws, "southern", 2).await;

    let (id_a, token_a) = send_ask(&node, &full, ws, &case_a, "northern").await;
    let (_id_b, token_b) = send_ask(&node, &full, ws, &case_b, "southern").await;

    // Withdrawn.
    call(
        &node,
        &full,
        ws,
        "case.request.withdraw",
        json!({ "id": id_a, "ts": T0 + 1 }),
    )
    .await
    .expect("withdraw ok");
    let withdrawn = lb_host::case_request_authenticate(&node.store, ws, &token_a, T0 + 2).await;
    assert_eq!(withdrawn.err(), Some(lb_host::RequestTokenError::Gone));

    // Expired — the same error value, byte for byte, so nothing downstream can distinguish them.
    let expired =
        lb_host::case_request_authenticate(&node.store, ws, &token_b, T0 + 3 * HOUR_MS).await;
    assert_eq!(expired.err(), Some(lb_host::RequestTokenError::Gone));

    // And a token that never existed is a different, equally opaque answer.
    let unknown =
        lb_host::case_request_authenticate(&node.store, ws, "lbr_nube.nonsense", T0).await;
    assert_eq!(unknown.err(), Some(lb_host::RequestTokenError::NotFound));
}

/// A replayed reply produces ONE event, one transition and one quote — the property a contractor on
/// a flaky signal depends on.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_replayed_reply_produces_exactly_one_event() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &full, ws, "replay-probe").await;
    seed_party(&node, &full, ws, "northern", 24).await;
    let (id, token) = send_ask(&node, &full, ws, &case_id, "northern").await;
    let (party, _) = lb_host::case_request_authenticate(&node.store, ws, &token, T0 + 1)
        .await
        .expect("resolves");

    let reply =
        json!({ "kind": "quote", "amount": 1180.0, "currency": "AUD", "text": "parts + labour" });
    let first = call(
        &node,
        &party,
        ws,
        "case.request.reply",
        json!({ "id": id, "reply": reply, "ts": T0 + 2 }),
    )
    .await
    .expect("first reply ok");
    assert_eq!(first["recorded"].as_bool(), Some(true));

    for _ in 0..3 {
        let again = call(
            &node,
            &party,
            ws,
            "case.request.reply",
            json!({ "id": id, "reply": reply, "ts": T0 + 3 }),
        )
        .await
        .expect("a replay must succeed, not error — the party retried a POST");
        assert_eq!(again["recorded"].as_bool(), Some(false));
    }

    let events = events_of(&node, &full, ws, &case_id).await;
    assert_eq!(
        count_kind(&events, "reply"),
        1,
        "four calls, one reply event: {events:#?}"
    );

    let case = call(&node, &full, ws, "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert_eq!(case["cost_to_fix"].as_f64(), Some(1180.0));
}

/// Each of the six reply kinds produces the documented `workflow` / `waiting_on` and an event
/// attributed to `party:{id}` — not to a login the contractor never had.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn every_reply_kind_moves_the_case_the_documented_way() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    seed_party(&node, &full, ws, "northern", 24).await;

    let expected = [
        (
            "accept",
            json!({ "kind": "accept" }),
            "actioned",
            "contractor",
        ),
        (
            "quote",
            json!({ "kind": "quote", "amount": 900.0, "currency": "AUD" }),
            "waiting_on_po",
            "client",
        ),
        (
            "eta",
            json!({ "kind": "eta", "eta_ts": T0 + 48 * HOUR_MS }),
            "actioned",
            "contractor",
        ),
        ("done", json!({ "kind": "done" }), "actioned", "internal"),
        (
            "need_info",
            json!({ "kind": "need_info", "text": "which floor?" }),
            "to_action",
            "internal",
        ),
        (
            "decline",
            json!({ "kind": "decline" }),
            "to_action",
            "internal",
        ),
    ];

    for (name, reply, workflow, waiting_on) in expected {
        let case_id = seed_case(&node, &full, ws, &format!("reply-{name}")).await;
        let (id, token) = send_ask(&node, &full, ws, &case_id, "northern").await;
        let (party, _) = lb_host::case_request_authenticate(&node.store, ws, &token, T0 + 1)
            .await
            .expect("resolves");
        call(
            &node,
            &party,
            ws,
            "case.request.reply",
            json!({ "id": id, "reply": reply, "ts": T0 + 2 }),
        )
        .await
        .unwrap_or_else(|e| panic!("{name} reply failed: {e:?}"));

        let case = call(&node, &full, ws, "case.get", json!({ "id": case_id }))
            .await
            .expect("get ok");
        assert_eq!(case["workflow"].as_str(), Some(workflow), "{name}");
        assert_eq!(case["waiting_on"].as_str(), Some(waiting_on), "{name}");
        assert_ne!(
            case["workflow"].as_str(),
            Some("resolved"),
            "{name}: no reply may close a case — that needs a resolution a party cannot give"
        );

        let events = events_of(&node, &full, ws, &case_id).await;
        let reply_event = events
            .iter()
            .find(|e| e["kind"].as_str() == Some("reply"))
            .unwrap_or_else(|| panic!("{name}: no reply event in {events:#?}"));
        assert_eq!(
            reply_event["actor"].as_str(),
            Some("party:northern"),
            "{name}: the reply is the PARTY's, not a login's"
        );
        assert_eq!(reply_event["data"]["kind"].as_str(), Some(name));
    }
}

// --- The nudge ladder ------------------------------------------------------------------------------

/// The scheduled nudges **actually fire** when the real reminder reactor is driven — and the whole
/// thing hangs off the sender's live grant, so the test first proves it goes RED without it
/// (`green-while-broken-reactor-tests.md`: a reactor test that never turns the cap off is green
/// over a broken wall).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_ladder_fires_under_the_senders_live_grant_and_stops_without_it() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &full, ws, "nudge-probe").await;
    seed_party(&node, &full, ws, "northern", 10).await;
    let (id, _token) = send_ask(&node, &full, ws, &case_id, "northern").await;

    // RED FIRST: the sender holds NO durable grant, so the fire-time re-resolve finds nothing and
    // the nudge is denied. Nothing is chased and nothing is counted.
    let at_50 = (T0 + 5 * HOUR_MS) / 1000;
    let pass = lb_host::react_to_reminders(&node, ws, at_50)
        .await
        .expect("a reactor pass");
    assert_eq!(
        pass.denied, 1,
        "an ungranted sender must be DENIED at fire time"
    );
    let listed = call(
        &node,
        &full,
        ws,
        "case.request.list",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("list ok");
    assert_eq!(
        listed.as_array().unwrap()[0]["nudges_sent"].as_u64(),
        Some(0)
    );

    // GREEN: grant the sender the cap the nudge rides on, and drive the 80 % rung.
    grant(&node.store, ws, "user:test", SEND).await;
    let at_80 = (T0 + 8 * HOUR_MS) / 1000;
    let pass = lb_host::react_to_reminders(&node, ws, at_80)
        .await
        .expect("a reactor pass");
    assert_eq!(pass.fired, 1, "the 80 % rung must fire: {pass:?}");

    let listed = call(
        &node,
        &full,
        ws,
        "case.request.list",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("list ok");
    assert_eq!(
        listed.as_array().unwrap()[0]["nudges_sent"].as_u64(),
        Some(1),
        "a fired nudge is counted on the row"
    );
    let events = events_of(&node, &full, ws, &case_id).await;
    assert_eq!(count_kind(&events, "nudge"), 1);

    // The chase is a real email, staged through the real outbox.
    let provider = drain_recording(&node, ws).await;
    let sends = provider.sends();
    assert_eq!(sends.len(), 1, "one nudge mail");
    assert!(
        !sends[0].body.contains("/r/"),
        "a nudge carries no link — the raw token is unrecoverable and re-minting would kill the \
         one already in their inbox: {}",
        sends[0].body
    );

    // The BREACH rung escalates rather than chasing: an event, no third email.
    let at_breach = (T0 + 10 * HOUR_MS) / 1000;
    lb_host::react_to_reminders(&node, ws, at_breach)
        .await
        .expect("a reactor pass");
    let events = events_of(&node, &full, ws, &case_id).await;
    assert_eq!(count_kind(&events, "breach"), 1, "{events:#?}");
    let provider = drain_recording(&node, ws).await;
    assert!(
        provider.sends().is_empty(),
        "the breach rung must not mail the party a third time"
    );

    // And the ask itself now reads expired rather than still waiting.
    let listed = call(
        &node,
        &full,
        ws,
        "case.request.list",
        json!({ "case_id": case_id }),
    )
    .await
    .expect("list ok");
    assert_eq!(
        listed.as_array().unwrap()[0]["status"].as_str(),
        Some("expired")
    );
    let _ = id;
}

/// Answering cancels the ladder: a nudge that fires afterwards chases nobody.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_reply_cancels_the_remaining_nudges() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    grant(&node.store, ws, "user:test", SEND).await;
    let case_id = seed_case(&node, &full, ws, "cancel-probe").await;
    seed_party(&node, &full, ws, "northern", 10).await;
    let (id, token) = send_ask(&node, &full, ws, &case_id, "northern").await;
    let (party, _) = lb_host::case_request_authenticate(&node.store, ws, &token, T0 + 1)
        .await
        .expect("resolves");
    call(
        &node,
        &party,
        ws,
        "case.request.reply",
        json!({ "id": id, "reply": { "kind": "accept" }, "ts": T0 + 2 }),
    )
    .await
    .expect("reply ok");

    let pass = lb_host::react_to_reminders(&node, ws, (T0 + 9 * HOUR_MS) / 1000)
        .await
        .expect("a reactor pass");
    assert_eq!(
        pass.fired, 0,
        "an answered ask must have no live rungs left: {pass:?}"
    );
    let events = events_of(&node, &full, ws, &case_id).await;
    assert_eq!(count_kind(&events, "nudge"), 0);
}

// --- Attachments ------------------------------------------------------------------------------------

/// A file the contractor uploads lands in the asset store under a ONE-SEGMENT id, and the reply
/// references it. A `.` in an asset id breaks `store:asset/{id}:write` — the upload succeeds and the
/// bytes are unreadable for ever.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_attachment_gets_a_single_segment_id_and_rides_on_the_reply() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &full, ws, "attach-probe").await;
    seed_party(&node, &full, ws, "northern", 24).await;
    let (id, token) = send_ask(&node, &full, ws, &case_id, "northern").await;
    let (party, _) = lb_host::case_request_authenticate(&node.store, ws, &token, T0 + 1)
        .await
        .expect("resolves");

    let receipt = lb_host::case_request_attach(
        &node.store,
        &party,
        ws,
        &id,
        // A filename full of the characters that would break a record id, to prove the id is NOT
        // derived from it.
        "site.photo.v2.jpg",
        "image/jpeg",
        b"\xff\xd8\xff-not-really-a-jpeg".to_vec(),
        T0 + 2,
    )
    .await
    .expect("attach ok");
    assert!(
        !receipt.id.contains('.') && !receipt.id.contains(':'),
        "an asset id must be one record-id segment: {}",
        receipt.id
    );
    assert_eq!(
        receipt.name, "site.photo.v2.jpg",
        "the name is data, not an id"
    );

    // The bytes really are readable back — the failure the one-segment rule prevents is silent.
    let stored = lb_assets::get_asset(&node.store, ws, &receipt.id)
        .await
        .expect("read ok")
        .expect("the asset exists");
    assert_eq!(stored.bytes.len(), receipt.size);

    call(
        &node,
        &party,
        ws,
        "case.request.reply",
        json!({
            "id": id,
            "reply": { "kind": "done", "text": "photo attached", "attachments": [receipt.id] },
            "ts": T0 + 3,
        }),
    )
    .await
    .expect("reply with an attachment ok");

    let events = events_of(&node, &full, ws, &case_id).await;
    let reply_event = events
        .iter()
        .find(|e| e["kind"].as_str() == Some("reply"))
        .expect("a reply event");
    assert_eq!(
        reply_event["data"]["attachments"][0].as_str(),
        Some(receipt.id.as_str())
    );

    // A token scoped to ANOTHER request cannot upload against this one.
    let case_b = seed_case(&node, &full, ws, "attach-other").await;
    seed_party(&node, &full, ws, "southern", 24).await;
    let (_id_b, token_b) = send_ask(&node, &full, ws, &case_b, "southern").await;
    let (other, _) = lb_host::case_request_authenticate(&node.store, ws, &token_b, T0 + 1)
        .await
        .expect("resolves");
    let denied = lb_host::case_request_attach(
        &node.store,
        &other,
        ws,
        &id,
        "sneak.jpg",
        "image/jpeg",
        b"nope".to_vec(),
        T0 + 4,
    )
    .await;
    assert!(
        denied.is_err(),
        "a token may only attach to its own request"
    );
}

// --- The roster -------------------------------------------------------------------------------------

/// `party.upsert` replaces in place and `party.list` narrows; a malformed party is refused before
/// anything is written.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_roster_upserts_in_place_narrows_and_refuses_a_bad_row() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let admin = principal("user:test", ws, ALL);

    seed_party(&node, &admin, ws, "northern", 24).await;
    seed_party(&node, &admin, ws, "northern", 48).await;
    let roster = call(&node, &admin, ws, "party.list", json!({}))
        .await
        .expect("list ok");
    let rows = roster.as_array().expect("an array");
    assert_eq!(rows.len(), 1, "an upsert replaces, never appends");
    assert_eq!(rows[0]["default_ask_window_h"].as_u64(), Some(48));

    // Narrowing by site and by kind.
    assert_eq!(
        call(&node, &admin, ws, "party.list", json!({ "site": "site-b" }))
            .await
            .expect("list ok")
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        call(&node, &admin, ws, "party.list", json!({ "kind": "client" }))
            .await
            .expect("list ok")
            .as_array()
            .map(Vec::len),
        Some(0)
    );

    // A contact that could never be mailed is refused at the door, not at 03:00 by the relay.
    let err = call(
        &node,
        &admin,
        ws,
        "party.upsert",
        json!({ "id": "broken", "kind": "fm", "name": "Broken", "contact": { "email": "not-an-address" } }),
    )
    .await
    .expect_err("a malformed email must be refused");
    assert!(matches!(err, ToolError::BadInput(_)), "{err:?}");
}

/// **The silent-drop regression.** A misplaced `email` (top level, not under `contact`) used to be
/// accepted: the write returned `{ "id": … }`, the address went nowhere, and the roster held a
/// contractor nobody could reach — the failure surfaced hours later at `case.request.send`, to a
/// different person. Both halves of the fix are pinned here: the door DENIES the unknown key, and a
/// party with no address is not a roster row at all.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_party_with_a_misplaced_or_missing_email_is_refused_loudly() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let admin = principal("user:test", ws, ALL);

    // 1. The misplaced key: `email` at the top level, exactly as an admin plausibly types it.
    let err = call(
        &node,
        &admin,
        ws,
        "party.upsert",
        json!({
            "id": "acme-mech",
            "name": "Acme Mechanical",
            "email": "quotes@acme-mech.example",
            "kind": "contractor",
        }),
    )
    .await
    .expect_err("a misplaced key must be refused, not silently dropped");
    assert!(matches!(err, ToolError::BadInput(_)), "{err:?}");

    // Nothing was written — the roster is exactly as it was.
    assert_eq!(
        call(&node, &admin, ws, "party.list", json!({}))
            .await
            .expect("list ok")
            .as_array()
            .map(Vec::len),
        Some(0),
        "a refused upsert must leave the roster untouched"
    );

    // 2. No contact at all: also refused, and the message says where the address goes.
    let err = call(
        &node,
        &admin,
        ws,
        "party.upsert",
        json!({ "id": "silent", "kind": "fm", "name": "Silent" }),
    )
    .await
    .expect_err("a party nobody can email is not a roster row");
    match err {
        ToolError::BadInput(m) => assert!(m.contains("contact"), "{m}"),
        other => panic!("{other:?}"),
    }

    // 3. Correctly placed, it works — so the deny is about the KEY, not about the value.
    call(
        &node,
        &admin,
        ws,
        "party.upsert",
        json!({
            "id": "acme-mech",
            "name": "Acme Mechanical",
            "kind": "contractor",
            "contact": { "email": "quotes@acme-mech.example" },
            // A `ts` rides along on every other case verb, so it is accepted and ignored here.
            "ts": T0,
        }),
    )
    .await
    .expect("the correct shape is accepted");
    let roster = call(&node, &admin, ws, "party.list", json!({}))
        .await
        .expect("list ok");
    assert_eq!(
        roster[0]["contact"]["email"].as_str(),
        Some("quotes@acme-mech.example")
    );
}

/// Defence in depth: a row written before the roster required an address — or by any path that is
/// not the verb — still cannot be asked anything, and the refusal names the reason.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_legacy_party_with_no_address_still_cannot_be_asked() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_roster(&node, ws).await;
    let admin = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &admin, ws, "no-email").await;

    // Straight into the store, bypassing the verb — the shape of a row that predates the rule.
    lb_store::write(
        &node.store,
        ws,
        "party",
        "legacy",
        &json!({ "id": "legacy", "kind": "fm", "name": "Legacy", "contact": {} }),
    )
    .await
    .expect("seed a legacy row");

    let err = call(
        &node,
        &admin,
        ws,
        "case.request.send",
        json!({ "case_id": case_id, "party_id": "legacy", "ask": "quote", "ts": T0 }),
    )
    .await
    .expect_err("an ask nobody receives is not an ask");
    match err {
        ToolError::BadInput(m) => assert!(m.contains("no email address"), "{m}"),
        other => panic!("{other:?}"),
    }
}
