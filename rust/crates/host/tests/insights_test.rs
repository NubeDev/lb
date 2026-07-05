//! The insights verbs over a REAL booted `Node` — real store, real bus, real caps, the real
//! `call_tool` MCP bridge (insights umbrella scope + sub-scopes). NO mocks (CLAUDE §9): records
//! are seeded by raising through the verb under test, then read back through it.
//!
//! **SKELETON**: every test below is NAMED for a mandatory or scope-named case + carries the
//! real-backend setup boilerplate (a booted `Node`, a `principal(...)` helper, the cap sets).
//! The bodies are `#[tokio::test] … todo!()` — `todo!()` so a green-but-lying stub is impossible.
//! The implementing session fills them in against the scope docs (each test names the case it
//! pins). The harness mirrors `channel_mcp_test.rs` / `approval_release_test.rs`.
//!
//! Mandatory categories (testing-scope §2): capability-deny (per verb) + workspace-isolation.
//! Scope-named cases: dedup-lifecycle, ring-cap, 2KB-reject, matcher-axes, ladder-escalate/decay,
//! breakthroughs, ack-suppression, digest-idempotency, kill-switch, determinism.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

/// Mint a principal carrying `caps` for `(sub, ws)`. Real signed token; the host's verify path
/// is exercised (the same shape every other host test uses).
fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// The full insight MCP surface caps a member holds (mirrors the dev-login set). Each verb is
/// also carried individually so a deny test can mint a principal WITHOUT just that one cap.
const RAISE: &str = "mcp:insight.raise:call";
const GET: &str = "mcp:insight.get:call";
const LIST: &str = "mcp:insight.list:call";
const ACK: &str = "mcp:insight.ack:call";
const RESOLVE: &str = "mcp:insight.resolve:call";
const OCC: &str = "mcp:insight.occurrences:call";
const SUB_CREATE: &str = "mcp:insight.sub.create:call";
const SUB_LIST: &str = "mcp:insight.sub.list:call";
const SUB_GET: &str = "mcp:insight.sub.get:call";
const SUB_DELETE: &str = "mcp:insight.sub.delete:call";
const SUB_MUTE: &str = "mcp:insight.sub.mute:call";
const POLICY_GET: &str = "mcp:insight.policy.get:call";
const POLICY_SET: &str = "mcp:insight.policy.set:call";
const CHAN_PUB: &str = "bus:chan/*:pub";

/// A caller holding the whole insight surface.
fn member_caps() -> Vec<&'static str> {
    vec![
        RAISE, GET, LIST, ACK, RESOLVE, OCC, SUB_CREATE, SUB_LIST, SUB_GET, SUB_DELETE, SUB_MUTE,
        POLICY_GET, CHAN_PUB,
    ]
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

/// The raise input fixture — a fraud-styled critical finding. Domain-free (rule 10): no provider
/// name; the dedup_key/body carry identity, never the title.
fn raise_input(dedup_key: &str, severity: &str, ts: u64) -> Value {
    json!({
        "dedup_key": dedup_key,
        "severity": severity,
        "title": "score above threshold",
        "origin": { "kind": "rule", "ref": "rule:scorer", "run": "job:1" },
        "tags": { "kind": "anomaly" },
        "occurrence": { "data": { "score": 0.93 }, "severity": severity },
        "ts": ts,
    })
}

// --- mandatory: capability deny (per verb) -----------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn raise_denied_without_the_cap() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let p = principal("user:bob", "acme", &[GET, LIST]); // no RAISE
    let r = call(
        &node,
        &p,
        "acme",
        "insight.raise",
        raise_input("k1", "critical", 1),
    )
    .await;
    assert!(matches!(r, Err(ToolError::Denied)));
    // SCOPE: insights-scope.md §"How it fits the core" → Capabilities. Body: assert no row was
    // written (a follow-up `insight.list` returns empty) once raise's body exists.
    todo!("insights: assert the deny left no record — SCOPE: insights-scope.md §Capabilities")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ack_denied_without_the_cap() {
    let _node = Arc::new(Node::boot().await.expect("node boots"));
    let _p = principal("user:bob", "acme", &[RAISE, GET, LIST]); // no ACK
                                                                 // SCOPE: insights-scope.md §"How it fits the core" → Capabilities.
    todo!("insights: raise, then ack is denied opaque — SCOPE: insights-scope.md §Capabilities")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn occurrences_denied_without_the_cap_even_with_get() {
    let _node = Arc::new(Node::boot().await.expect("node boots"));
    let _p = principal("user:bob", "acme", &[RAISE, GET]); // no OCC
                                                           // SCOPE: insight-occurrences-scope.md §"How it fits the core" → Capabilities.
    todo!("insights: raise, then occurrences denied even with insight.get — SCOPE: occurrences-scope.md §Capabilities")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn sub_create_denied_without_the_channel_pub() {
    let _node = Arc::new(Node::boot().await.expect("node boots"));
    let _p = principal("user:bob", "acme", &[SUB_CREATE]); // no bus:chan/*:pub
                                                           // SCOPE: insight-subscriptions-scope.md §"How it fits the core" → Capabilities (no-widening up front).
    todo!("insights: sub.create denied when the caller lacks bus:chan/<channel>:pub — SCOPE: subscriptions-scope.md §Capabilities")
}

// --- mandatory: workspace isolation ------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_in_one_workspace_never_returns_another_workspaces_insights() {
    let _node = Arc::new(Node::boot().await.expect("node boots"));
    let _a = principal("user:ada", "ws-a", &member_caps());
    let _b = principal("user:bea", "ws-b", &member_caps());
    // SCOPE: insights-scope.md §"How it fits the core" → Tenancy/isolation.
    todo!("insights: raise in ws-A, assert ws-B list is empty (no leak) — SCOPE: insights-scope.md §Tenancy")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn occurrences_never_leak_across_workspaces() {
    let _node = Arc::new(Node::boot().await.expect("node boots"));
    let _a = principal("user:ada", "ws-a", &member_caps());
    let _b = principal("user:bea", "ws-b", &member_caps());
    // SCOPE: insight-occurrences-scope.md §"How it fits the core" → Tenancy.
    todo!("insights: raise in ws-A, ws-B occurrences call cannot read it — SCOPE: occurrences-scope.md §Tenancy")
}

// --- scope-named: dedup lifecycle --------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn raise_dedup_bumps_count_and_preserves_acked_status() {
    let _node = Arc::new(Node::boot().await.expect("node boots"));
    let _p = principal("user:ada", "ws-a", &member_caps());
    // SCOPE: insights-scope.md §"Dedup / flap suppression".
    // raise → count=1 open; raise same key → count=2 open; ack; raise again → count=3 acked
    // (status UNTOUCHED — an acked fault re-firing doesn't re-page anyone).
    todo!("insights: dedup bumps count, preserves acked status — SCOPE: insights-scope.md §Dedup")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn raise_after_resolve_reopens() {
    let _node = Arc::new(Node::boot().await.expect("node boots"));
    let _p = principal("user:ada", "ws-a", &member_caps());
    // SCOPE: insights-scope.md §"Dedup / flap suppression" → "Existing resolved → re-open".
    todo!("insights: resolved → raise again re-opens (status=open, count continues) — SCOPE: insights-scope.md §Dedup")
}

// --- scope-named: occurrence ring --------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ring_cap_evicts_oldest_but_count_is_lifetime() {
    let _node = Arc::new(Node::boot().await.expect("node boots"));
    let _p = principal("user:ada", "ws-a", &member_caps());
    // SCOPE: insight-occurrences-scope.md §"The record" → ring semantics. cap+1 raises → cap
    // rows, oldest evicted, `count` = cap+1.
    todo!(
        "insights: cap+1 raises → cap rows + count=cap+1 — SCOPE: occurrences-scope.md §The record"
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn oversize_occurrence_data_rejects_the_whole_raise() {
    let _node = Arc::new(Node::boot().await.expect("node boots"));
    let _p = principal("user:ada", "ws-a", &member_caps());
    // SCOPE: insight-occurrences-scope.md §"The record" → 2 KB+1 `data` → whole raise rejected
    // BadInput (never silent truncation); exactly-2 KB accepted.
    todo!("insights: 2KB+1 occurrence.data rejects the raise — SCOPE: occurrences-scope.md §The record")
}

// --- scope-named: matcher axes -----------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn matcher_returns_one_intent_per_matching_sub() {
    // SCOPE: insight-subscriptions-scope.md §"The raise-time matcher". Each axis alone + AND
    // composition + empty filter = all + tag-subset + severity ordering + non-match = nothing.
    // This drives the pure `lb_insights::match_subs` through the real raise path; the pure-fn
    // unit tests live in `crates/insights/tests/ladder_test.rs`.
    todo!("insights: matcher AND-filter + tag-subset + severity floor — SCOPE: subscriptions-scope.md §The raise-time matcher")
}

// --- scope-named: ladder (escalate/decay/breakthroughs/ack-suppress) ---------------------
//
// NOTE: the ladder state machine is PURE (`lb_insights::ladder_step`) — its unit tests live in
// `crates/insights/tests/ladder_test.rs` (zero I/O, deterministic). THIS file's ladder test is
// the INTEGRATION headline: raise ×10 through the real raise path → assert one immediate + one
// hourly digest (the notify scope's "5-minute nag, tamed" example flow).

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ladder_escalates_under_sustained_noise() {
    let _node = Arc::new(Node::boot().await.expect("node boots"));
    let _p = principal("user:ada", "ws-a", &member_caps());
    // SCOPE: insight-notify-scope.md §"Example flow (the 5-minute nag, tamed)".
    todo!("insights: raise ×10 within the window escalates L0→L1 (one immediate + pending digest) — SCOPE: notify-scope.md §Example flow")
}

// --- scope-named: digest idempotency -----------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn digest_reactor_is_idempotent_on_rerun() {
    let _node = Arc::new(Node::boot().await.expect("node boots"));
    let _p = principal("user:ada", "ws-a", &member_caps());
    // SCOPE: insight-notify-scope.md §"The digest reactor". Re-running with the same `now` re-
    // upserts the same digest item (same id) — no duplicate channel.post.
    todo!("insights: digest reactor idempotent per (sub, window_start) — SCOPE: notify-scope.md §The digest reactor")
}

// --- scope-named: kill switch ------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn member_kill_switch_off_skips_all_deliveries() {
    let _node = Arc::new(Node::boot().await.expect("node boots"));
    let _p = principal("user:ada", "ws-a", &member_caps());
    // SCOPE: insight-notify-scope.md §"Settings surface" → per-member kill switch. Prefs axis
    // `insight_notifications: Some(false)` ⇒ nothing posts; flip back ⇒ next window digests
    // include the gap (one summary, not N — no replay flood).
    todo!("insights: prefs kill switch off ⇒ no deliveries; on ⇒ resumes — SCOPE: notify-scope.md §Settings surface")
}
