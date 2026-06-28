//! Runtime outbox/store taint (undo scope: "classification is runtime transaction taint"). The taint
//! is what auto-capture-on-dispatch uses to classify a tool call from what it *actually did* — so the
//! load-bearing property is **composition**: a nested reach taints the enclosing action as a whole
//! (the `max` rule). Proven here without the host, against the real task-local mechanism.

use lb_store::{
    mark_outbox_reached, mark_store_written, outbox_was_reached, store_was_written, taint_scope,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_untainted_scope_reports_clean() {
    let ((), v) = taint_scope(async {}).await;
    assert!(!v.reached_outbox, "no outbox reach");
    assert!(!v.wrote_store, "no store write");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn marking_inside_a_scope_is_observed() {
    let ((), v) = taint_scope(async {
        mark_store_written();
        mark_outbox_reached();
    })
    .await;
    assert!(v.reached_outbox);
    assert!(v.wrote_store);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_nested_outbox_reach_taints_the_enclosing_action() {
    // The composition `max` rule: an OUTER action that itself only writes state, but whose NESTED
    // call reaches the outbox, is tainted irreversible AS A WHOLE — exactly the footgun the scope
    // exists to prevent (a "reversible" tool whose nested call silently enqueues an effect).
    let ((), outer) = taint_scope(async {
        mark_store_written(); // the outer, reversible-looking work

        // A nested call (same task → shares the enclosing cell). It reaches the outbox.
        let ((), inner) = taint_scope(async {
            mark_outbox_reached();
        })
        .await;
        // The nested scope already sees the reach (it shares the enclosing cell).
        assert!(inner.reached_outbox, "nested scope sees the reach");
    })
    .await;

    assert!(
        outer.reached_outbox,
        "the nested outbox reach bubbled up to taint the enclosing action"
    );
    assert!(outer.wrote_store);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn marks_outside_any_scope_are_silent_no_ops() {
    // The raw store/outbox path is unaffected when no dispatch scope is open.
    mark_store_written();
    mark_outbox_reached();
    assert!(!store_was_written(), "no scope → silent no-op");
    assert!(!outbox_was_reached(), "no scope → silent no-op");
}
