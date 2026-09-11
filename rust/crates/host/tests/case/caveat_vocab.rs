//! (f) The workspace-declared category vocabulary — unseeded validates nothing, seeded names the set it rejected against.
//!
//! Part of the `caveat` suite (see `caveat_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::caveat_support::*;

// --- (f) the workspace-declared category vocabulary ----------------------------------------------

/// **lb ships no default value list.** A workspace nobody seeded accepts anything, exactly as it did
/// before this validation existed — so adding it breaks no existing workspace and encodes no
/// product taxonomy in lb (rule 10).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_unseeded_workspace_validates_nothing() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    let p = principal("user:test", ws, &caps());

    let out = raise(
        &node,
        &p,
        ws,
        raise_input("k1", "warning", 1, Some("anything-at-all"), &[]),
    )
    .await;
    assert!(out.is_ok(), "an unseeded vocabulary is OPEN: {out:?}");
}

/// A seeded workspace rejects a value outside its declared set — **and names the set**, so a rule
/// author sees what they may have meant instead of a bare rejection. And the whole raise is
/// rejected: no record lands with a value nothing can group by.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_seeded_workspace_rejects_an_undeclared_value_and_names_the_set() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_vocab(&node, ws).await;
    let p = principal("user:test", ws, &caps());

    let err = raise(
        &node,
        &p,
        ws,
        raise_input("k1", "warning", 1, Some("nonsense"), &[]),
    )
    .await
    .expect_err("outside the declared set");
    let msg = format!("{err:?}");
    assert!(msg.contains("nonsense"), "names the offending value: {msg}");
    for v in VALUES {
        assert!(
            msg.contains(v),
            "names the declared set ({v} missing): {msg}"
        );
    }

    // Nothing was written — the guard runs before any store write.
    let page = call(&node, &p, ws, "insight.list", json!({}))
        .await
        .expect("list ok");
    assert!(
        page["items"].as_array().expect("items").is_empty(),
        "a rejected category leaves no partial record: {page}"
    );

    // A declared value goes straight through.
    assert!(raise(
        &node,
        &p,
        ws,
        raise_input("k1", "warning", 1, Some(VALUES[1]), &[])
    )
    .await
    .is_ok());
}
