//! `lb-supervisor` — the multiplexed control line: **throughput and reply demultiplexing** (native-call-concurrency scope).
//!
//! Harness: `concurrent_child.rs` — a deliberately CONCURRENT and slow fake child (spawns each
//! handler, one writer task), because a fake that answers instantly or serially makes every
//! assertion here pass whether or not the transport multiplexes. Each test states what it was
//! revert-checked against.

#[path = "concurrent_child.rs"]
mod concurrent_child;
use concurrent_child::*;

/// **Test 1 — concurrency is real, measured structurally.**
///
/// 13 calls whose handler sleeps `WORK` each. Serial would take 13 × 100 ms = 1.3 s; concurrent
/// takes ~100 ms. The bound is deliberately far below the serial figure so this cannot pass on a
/// slow machine that merely got lucky.
///
/// REVERT-CHECK: re-serializing the host (holding the sidecar guard across the round-trip in
/// `native/call.rs`, or awaiting each `request` here) takes this to ~1.3 s → RED.
#[tokio::test]
async fn thirteen_calls_overlap_instead_of_queueing() {
    let (l, _) = launcher();
    let sc = Arc::new(Sidecar::spawn(Spec::new("fake"), &l).await.unwrap());

    let start = Instant::now();
    let mut tasks = Vec::new();
    for i in 0..13u32 {
        let sc = Arc::clone(&sc);
        tasks.push(tokio::spawn(async move {
            sc.call("q", &i.to_string()).await.unwrap()
        }));
    }
    let mut outs = Vec::new();
    for t in tasks {
        outs.push(t.await.unwrap());
    }
    let elapsed = start.elapsed();

    assert_eq!(outs.len(), 13);
    assert!(
        elapsed < WORK * 4,
        "13 concurrent calls took {elapsed:?}; serial would be ~{:?}. The line is not multiplexed.",
        WORK * 13
    );
}

/// **Test 2 — replies are correctly demultiplexed (per-caller IDENTITY, not mere success).**
///
/// This is the failure mode the design introduces: every caller getting a valid-but-wrong answer
/// looks exactly like success. So each caller sends a DISTINCT input and must receive ITS OWN input
/// echoed back — asserting the pairing, not the shape.
///
/// REVERT-CHECK: restoring `if reply.id != id { continue }` as a filter (each caller reading the
/// stream and discarding non-matching ids) makes callers steal each other's replies → RED.
#[tokio::test]
async fn each_caller_receives_its_own_reply() {
    let (l, _) = launcher();
    let sc = Arc::new(Sidecar::spawn(Spec::new("fake"), &l).await.unwrap());

    let mut tasks = Vec::new();
    for i in 0..13u32 {
        let sc = Arc::clone(&sc);
        tasks.push(tokio::spawn(async move {
            let out = sc.call("q", &i.to_string()).await.unwrap();
            (i, out)
        }));
    }

    for t in tasks {
        let (sent, got) = t.await.unwrap();
        let v: serde_json::Value = serde_json::from_str(&got).unwrap();
        assert_eq!(
            v["echo"].as_u64().unwrap(),
            sent as u64,
            "caller {sent} received another caller's reply: {got}"
        );
    }
}

/// **Test 2b — the per-call `caller` stamp survives multiplexing.**
///
/// Caller identity is per-frame, never connection state. With N concurrent calls carrying different
/// principals, each child-side frame must see the principal that actually made THAT call — a leak
/// here would be an authorization-relevant misattribution, not just wrong data.
#[tokio::test]
async fn each_caller_identity_stays_with_its_own_call() {
    let (l, _) = launcher();
    let sc = Arc::new(Sidecar::spawn(Spec::new("fake"), &l).await.unwrap());

    let mut tasks = Vec::new();
    for i in 0..8u32 {
        let sc = Arc::clone(&sc);
        tasks.push(tokio::spawn(async move {
            let caller = lb_supervisor::Caller {
                sub: format!("user:{i}"),
                ws: "acme".into(),
                role: "member".into(),
                delegated: false,
                admin: false,
            };
            let out = sc
                .call_with_caller("q", &i.to_string(), Some(caller))
                .await
                .unwrap();
            (i, out)
        }));
    }

    for t in tasks {
        let (sent, got) = t.await.unwrap();
        let v: serde_json::Value = serde_json::from_str(&got).unwrap();
        assert_eq!(
            v["caller"].as_str().unwrap(),
            format!("user:{sent}"),
            "call from user:{sent} was stamped with another caller's identity: {got}"
        );
    }
}
