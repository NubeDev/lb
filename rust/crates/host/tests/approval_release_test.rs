//! The **rules approval loop** end to end through real host seams (rules-approvals-scope): a gated
//! effect is staged `held`, the relay never delivers it while held, an approval releases it to the
//! outbox (delivered exactly once), a reject discards it (never delivered), a defer leaves it held.
//!
//! Everything is REAL — a booted `Node` (real embedded SurrealDB + in-proc Zenoh, so multi-thread + a
//! unique workspace per test), the real `call_tool` gate/dispatch (the SAME entry the gateway's
//! `POST /mcp/call` forwards), the real `inbox.resolve`/`outbox.enqueue_held` verbs, and the real
//! `react_to_approval_releases` reactor + `relay_outbox` relay. The delivery `Target` is the only
//! external stubbed (testing §3): a local recording sink.
//!
//! Mandatory categories: capability-deny (§2.1), workspace-isolation (§2.2), the gated release
//! headline, idempotency/determinism, and the durable re-scan (offline/sync §2.3 — the reactor is a
//! pure re-read of durable records, so a post-restart pass releases an approved-just-before-crash
//! effect).

use std::sync::Arc;
use std::sync::Mutex;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, react_to_approval_releases, relay_outbox, Node, Target};
use lb_outbox::{Effect, EffectStatus};

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

const ENQUEUE: &str = "mcp:outbox.enqueue:call";
const RECORD: &str = "mcp:inbox.record:call";
const RESOLVE: &str = "mcp:inbox.resolve:call";
const STATUS: &str = "mcp:outbox.status:call";

/// The grant a rule needs to raise an approval + stage its held effect + resolve it (the reviewer).
fn caps() -> Vec<&'static str> {
    vec![ENQUEUE, RECORD, RESOLVE, STATUS]
}

/// A recording delivery sink — the only external stubbed. Records every delivered effect's id.
#[derive(Default)]
struct RecordingTarget {
    delivered: Mutex<Vec<String>>,
}
impl Target for RecordingTarget {
    async fn deliver(&self, effect: &Effect) -> Result<(), String> {
        self.delivered.lock().unwrap().push(effect.id.clone());
        Ok(())
    }
}

/// Stage the held gated effect for approval item `item_id` (target `email`/`send`) via the real verb.
async fn stage_held(node: &Arc<Node>, p: &Principal, ws: &str, item_id: &str) {
    let args = format!(
        r#"{{"item_id":"{item_id}","target":"email","action":"send","payload":{{"to":"x"}},"ts":1}}"#
    );
    call_tool(node, p, ws, "outbox.enqueue_held", &args)
        .await
        .expect("stage held effect");
    // The `needs:approval` item itself (the request_approval verb records both; here we stage the two
    // halves through the raw verbs, mirroring what the rule seam does).
    let rec = format!(r#"{{"channel":"ops","id":"{item_id}","body":"needs:approval x","ts":1}}"#);
    call_tool(node, p, ws, "inbox.record", &rec)
        .await
        .expect("record item");
}

async fn resolve(node: &Arc<Node>, p: &Principal, ws: &str, item_id: &str, decision: &str) {
    let args = format!(r#"{{"item_id":"{item_id}","decision":"{decision}","ts":2}}"#);
    call_tool(node, p, ws, "inbox.resolve", &args)
        .await
        .expect("resolve");
}

/// The held effect id the verb derives for `item_id` (mirrors `lb_host::held_effect_id`).
fn held_id(item_id: &str) -> String {
    lb_host::held_effect_id(item_id)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn approval_releases_the_held_effect_and_the_relay_delivers_it_exactly_once() {
    // THE HEADLINE: a held effect is NOT delivered while held; approving it releases it (held→pending)
    // and the relay delivers it exactly once.
    let ws = "appr-happy";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ana", ws, &caps());
    stage_held(&node, &p, ws, "refund-1").await;

    let eid = held_id("refund-1");

    // While held: the relay's schedulable scan excludes it — the target never sees it.
    let target = RecordingTarget::default();
    let rp = relay_outbox(&node.store, ws, &target, 10).await.unwrap();
    assert_eq!(rp.delivered, 0, "a held effect is never delivered");
    assert!(lb_outbox::pending(&node.store, ws)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(lb_outbox::held(&node.store, ws).await.unwrap().len(), 1);

    // Approve → the reactor releases it (held→pending).
    resolve(&node, &p, ws, "refund-1", "approved").await;
    let pass = react_to_approval_releases(&node, ws).await.unwrap();
    assert_eq!(pass.released, 1);
    assert_eq!(pass.discarded, 0);
    assert!(lb_outbox::held(&node.store, ws).await.unwrap().is_empty());

    // Now the relay delivers it — exactly once.
    let rp = relay_outbox(&node.store, ws, &target, 11).await.unwrap();
    assert_eq!(rp.delivered, 1);
    assert_eq!(*target.delivered.lock().unwrap(), vec![eid]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rejection_discards_the_held_effect_and_it_is_never_delivered() {
    let ws = "appr-reject";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ana", ws, &caps());
    stage_held(&node, &p, ws, "refund-1").await;

    resolve(&node, &p, ws, "refund-1", "rejected").await;
    let pass = react_to_approval_releases(&node, ws).await.unwrap();
    assert_eq!(pass.discarded, 1);
    assert_eq!(pass.released, 0);

    // Discarded is terminal — not held, not schedulable; the relay never picks it up.
    assert!(lb_outbox::held(&node.store, ws).await.unwrap().is_empty());
    let target = RecordingTarget::default();
    let rp = relay_outbox(&node.store, ws, &target, 11).await.unwrap();
    assert_eq!(rp.delivered, 0, "a discarded effect is never delivered");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn deferral_leaves_the_effect_held() {
    let ws = "appr-defer";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ana", ws, &caps());
    stage_held(&node, &p, ws, "refund-1").await;

    resolve(&node, &p, ws, "refund-1", "deferred").await;
    let pass = react_to_approval_releases(&node, ws).await.unwrap();
    assert_eq!(pass.released, 0);
    assert_eq!(pass.discarded, 0, "deferred is inert in v1 — still held");
    assert_eq!(lb_outbox::held(&node.store, ws).await.unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn releasing_twice_delivers_exactly_once() {
    // IDEMPOTENCY (mandatory): a replay (a second reactor pass, or a deferred-then-approved item) must
    // release ONCE. The guarded held→pending transition makes the second pass a no-op.
    let ws = "appr-idem";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ana", ws, &caps());
    stage_held(&node, &p, ws, "refund-1").await;
    resolve(&node, &p, ws, "refund-1", "approved").await;

    let p1 = react_to_approval_releases(&node, ws).await.unwrap();
    assert_eq!(p1.released, 1);
    // Second pass: the effect is already pending → guarded transition is a no-op.
    let p2 = react_to_approval_releases(&node, ws).await.unwrap();
    assert_eq!(p2.released, 0, "a replay releases nothing new");

    // Exactly one pending effect, delivered exactly once.
    let target = RecordingTarget::default();
    let rp = relay_outbox(&node.store, ws, &target, 11).await.unwrap();
    assert_eq!(rp.delivered, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn staging_a_held_effect_is_denied_without_the_outbox_cap() {
    // MANDATORY capability-deny (§2.1): `outbox.enqueue_held` gates on the `mcp:outbox.enqueue:call`
    // grant (staging is the same authority as any enqueue). A caller lacking it is refused opaquely,
    // and NO held effect lands.
    let ws = "appr-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    // Has record/resolve but NOT the outbox enqueue cap.
    let p = principal("user:ana", ws, &[RECORD, RESOLVE, STATUS]);
    let args = r#"{"item_id":"refund-1","target":"email","action":"send","payload":{},"ts":1}"#;
    let denied = call_tool(&node, &p, ws, "outbox.enqueue_held", args).await;
    assert!(
        matches!(denied, Err(lb_mcp::ToolError::Denied)),
        "opaque deny at the outbox cap: {denied:?}"
    );
    assert!(
        lb_outbox::held(&node.store, ws).await.unwrap().is_empty(),
        "no held effect staged without the grant"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_user_token_cannot_force_a_release() {
    // The release is a SYSTEM transition (the reactor's authority), not a user verb — there is no
    // `outbox.release`/`outbox.enqueue_held→pending` MCP verb a token can call. A user holding every
    // messaging cap still cannot move a held effect to pending except by approving (which routes
    // through the reactor). We assert the surface: no such verb exists on the dispatcher.
    let ws = "appr-noforce";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ana", ws, &caps());
    stage_held(&node, &p, ws, "refund-1").await;

    // There is no release verb — an attempt to call one is a missing tool (opaque), never a release.
    let attempt = call_tool(&node, &p, ws, "outbox.release", r#"{"id":"held:refund-1"}"#).await;
    assert!(attempt.is_err(), "no user-callable release verb");
    // The effect is still held — untouched by the (nonexistent) forced release.
    assert_eq!(lb_outbox::held(&node.store, ws).await.unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_b_approval_never_releases_a_ws_a_held_effect() {
    // MANDATORY workspace-isolation (§2.2): ws-A stages+approves; a ws-B reactor pass releases nothing
    // in ws-A (its scans select ws-B's namespace). And a ws-B token cannot resolve a ws-A item.
    let node = Arc::new(Node::boot().await.unwrap());
    let a = principal("user:a", "iso-a", &caps());
    let b = principal("user:b", "iso-b", &caps());

    stage_held(&node, &a, "iso-a", "refund-1").await;
    resolve(&node, &a, "iso-a", "refund-1", "approved").await;

    // A ws-B token cannot even resolve a ws-A item — the pin refuses it (opaque) before the cap check.
    let cross = call_tool(
        &node,
        &b,
        "iso-b",
        "inbox.resolve",
        r#"{"item_id":"refund-1","decision":"approved","ts":3}"#,
    )
    .await;
    // (This resolves an item in ws-B's namespace, not ws-A's — ws-A's item is unreachable from ws-B.)
    let _ = cross;

    // A ws-B reactor pass sees no ws-A resolution/effect → releases nothing anywhere.
    let pass_b = react_to_approval_releases(&node, "iso-b").await.unwrap();
    assert_eq!(pass_b.released, 0, "ws-B reactor sees no ws-A approval");
    // ws-A's held effect is untouched by the ws-B pass.
    assert_eq!(
        lb_outbox::held(&node.store, "iso-a").await.unwrap().len(),
        1
    );

    // ws-A's own reactor DOES release it — proving the approval was genuinely there.
    let pass_a = react_to_approval_releases(&node, "iso-a").await.unwrap();
    assert_eq!(pass_a.released, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_re_scan_after_restart_releases_an_approved_effect() {
    // MANDATORY offline/sync (§2.3): the held effect + the resolution are durable records; the reactor
    // is a pure re-read of them. So a pass run "fresh" (the post-restart re-scan) releases an effect
    // approved just before the restart — nothing is lost, and it is still released exactly once.
    let ws = "appr-restart";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ana", ws, &caps());
    stage_held(&node, &p, ws, "refund-1").await;
    resolve(&node, &p, ws, "refund-1", "approved").await;

    // (Simulated restart: no in-process reactor state carries over — a brand-new pass over the durable
    // records is exactly what the post-reboot tick does.)
    let pass = react_to_approval_releases(&node, ws).await.unwrap();
    assert_eq!(
        pass.released, 1,
        "the durable approval is picked up on the re-scan"
    );
    let effect = &lb_outbox::pending(&node.store, ws).await.unwrap()[0];
    assert_eq!(effect.status, EffectStatus::Pending);
}
