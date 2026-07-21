//! `lb-supervisor` — the multiplexed control line: **lifecycle across a channel generation** (native-call-concurrency scope).
//!
//! Harness: `concurrent_child.rs` — a deliberately CONCURRENT and slow fake child (spawns each
//! handler, one writer task), because a fake that answers instantly or serially makes every
//! assertion here pass whether or not the transport multiplexes. Each test states what it was
//! revert-checked against.

#[path = "concurrent_child.rs"]
mod concurrent_child;
use concurrent_child::*;

/// **Test 5 — the restart generation boundary (the id-collision hazard), constructed deliberately.**
///
/// `restart`/`rearm` reset the id allocator to 0. Under a pending map shared across generations, a
/// pre-restart waiter on id 3 would be woken by the RESTARTED child's id 3 — which belongs to a
/// different caller. Valid JSON, wrong rows, no error.
///
/// This will NOT appear by accident in a test that restarts a quiet child, so it is built on purpose:
/// launch N calls, restart while they are in flight, and assert every one of them fails with a
/// transport error rather than being answered — in particular that none is answered by generation 1.
/// The fake stamps `gen` into each reply precisely so a cross-generation answer is detectable.
#[tokio::test]
async fn a_restart_never_lets_the_new_child_answer_an_old_waiter() {
    let (l, _) = launcher();
    let sc = Arc::new(tokio::sync::Mutex::new(
        Sidecar::spawn(Spec::new("fake"), &l).await.unwrap(),
    ));

    // Detach the live generation, then fire calls that will still be in flight during the restart.
    let conn = sc.lock().await.conn().unwrap();
    let mut inflight = Vec::new();
    for i in 0..6u32 {
        let conn = Arc::clone(&conn);
        inflight.push(tokio::spawn(async move {
            conn.call_with_caller("q", &i.to_string(), None).await
        }));
    }

    // Let the frames land, but restart well before the handlers finish (WORK = 100 ms).
    tokio::time::sleep(Duration::from_millis(20)).await;
    sc.lock().await.restart(&l).await.expect("restarts");

    for t in inflight {
        let result = t.await.unwrap();
        match result {
            Err(SupervisorError::Transport(_)) => {} // correct: the generation died under it
            Err(other) => panic!("expected a transport error across the boundary, got {other:?}"),
            Ok(out) => panic!(
                "a pre-restart waiter was ANSWERED across a generation boundary: {out}. \
                 This is the id-collision bug: the reply belongs to a different caller."
            ),
        }
    }

    // The restarted child is live and serving on a fresh generation.
    let conn2 = sc.lock().await.conn().unwrap();
    let out = conn2.call_with_caller("q", "99", None).await.unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["echo"].as_u64().unwrap(), 99);
    assert_eq!(v["gen"].as_u64().unwrap(), 1, "should be generation 1");
}

/// **Test 3 (reader-exit half) — no waiter is ever orphaned.**
///
/// If the reader task exits (child death, decode error) without draining, every waiter hangs
/// *silently* forever — strictly worse than the failure it replaces. Every in-flight caller must be
/// woken with a transport error.
///
/// The whole test is wrapped in a timeout: an orphaned-waiter regression manifests as a HANG, and a
/// hanging test that never fails is indistinguishable from a passing one in CI.
#[tokio::test]
async fn child_death_wakes_every_waiter_with_an_error() {
    let (l, _) = launcher();
    let mut sc = Sidecar::spawn(Spec::new("fake"), &l).await.unwrap();
    let conn = sc.conn().unwrap();

    let mut inflight = Vec::new();
    for i in 0..6u32 {
        let conn = Arc::clone(&conn);
        inflight.push(tokio::spawn(async move {
            conn.call_with_caller("q", &i.to_string(), None).await
        }));
    }
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Shut the sidecar down under the in-flight calls.
    sc.shutdown().await;

    for t in inflight {
        let result = tokio::time::timeout(Duration::from_secs(5), t)
            .await
            .expect("a waiter HUNG instead of being failed — the orphaned-waiter bug")
            .unwrap();
        assert!(
            result.is_err(),
            "a waiter was satisfied after the connection closed: {result:?}"
        );
    }
}

/// **Test 7 — no task is leaked across repeated restarts.**
///
/// Each generation starts a reader task; if the outgoing one is not stopped, restarts accumulate
/// tasks that hold a dead pipe forever. Asserted structurally: after N restarts the live task count
/// must not have grown by N.
#[tokio::test]
async fn repeated_restarts_do_not_leak_reader_tasks() {
    let (l, _) = launcher();
    let mut spec = Spec::new("fake");
    spec.backoff.max_restarts = 50;
    let mut sc = Sidecar::spawn(spec, &l).await.unwrap();

    // Hold each generation alive across its own restart and assert it was actually CLOSED.
    //
    // Asserting on `num_alive_tasks` alone was VACUOUS: `Conn::drop` aborts the reader, so a
    // generation that `restart` forgot to close still got cleaned up the moment the last `Arc` went
    // away, and the count never grew. (Verified — that version passed against a deliberately
    // leaking `restart`.) The observable property is that the OLD generation is dead *while still
    // referenced*: a call on it must be refused, which is only true if `restart` closed it.
    let baseline = tokio::runtime::Handle::current()
        .metrics()
        .num_alive_tasks();
    let mut retired = Vec::new();
    for _ in 0..10 {
        let old = sc.conn().unwrap();
        sc.restart(&l).await.expect("restart");
        assert!(
            old.call_with_caller("q", "0", None).await.is_err(),
            "a superseded generation still served a call — restart did not close it, \
             so its reader task and pending map leak for as long as anything holds it"
        );
        retired.push(old);
    }
    // Now drop them all and confirm nothing accumulated.
    drop(retired);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let after = tokio::runtime::Handle::current()
        .metrics()
        .num_alive_tasks();

    assert!(
        after <= baseline + 4,
        "reader tasks leaked across restarts: {baseline} → {after} after 10 restarts"
    );

    // And it still serves.
    let out = sc.call("q", "1").await.unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&out).unwrap()["echo"]
            .as_u64()
            .unwrap(),
        1
    );
}

/// **Test 8 — lifecycle verbs still work with the reader task owning the read half.**
///
/// `init` (in `spawn`), `health`, and `shutdown` each go through a different path now: `init` runs
/// raw before the reader exists, `health` is an ordinary correlated request, and `shutdown` must
/// register a waiter and get its reply rather than reading the wire itself.
#[tokio::test]
async fn init_health_and_shutdown_all_complete_under_the_reader() {
    let (l, _) = launcher();
    let mut sc = Sidecar::spawn(Spec::new("fake"), &l).await.unwrap();

    sc.health().await.expect("health under the reader task");

    // health must not queue behind saturated calls: fire work, then poll while it is in flight.
    let conn = sc.conn().unwrap();
    let busy: Vec<_> = (0..8u32)
        .map(|i| {
            let conn = Arc::clone(&conn);
            tokio::spawn(async move { conn.call_with_caller("q", &i.to_string(), None).await })
        })
        .collect();
    let polled = Instant::now();
    sc.health().await.expect("health while calls are in flight");
    assert!(
        polled.elapsed() < WORK,
        "health queued behind in-flight calls ({:?})",
        polled.elapsed()
    );
    for b in busy {
        let _ = b.await;
    }

    sc.shutdown().await;
    assert!(sc.call("q", "1").await.is_err(), "dead after shutdown");
}
