//! The vocabulary WRITE door — `insight.vocab.list` / `insight.vocab.set`
//! (case-plane scope §"Vocabulary").
//!
//! Part of the `caveat` suite (see `caveat_support.rs` for the fixtures). One binary: `case_suite.rs`.
//!
//! The door exists because the vocabulary is an ENGINE-OWNED record — `raise` refuses a category
//! outside `values`, the caveat stamp reads `gates` — and until now a workspace could only author
//! one through the generic `store.write`, which is a UI writing a record the engine then has to
//! trust. These tests are about what that door REFUSES; the validation is the whole point of having
//! it rather than a raw write.
//!
//! Every vocabulary value here is seeded BY THIS TEST as workspace data (`caveat_support::VALUES`).
//! Grep the crate for one and you find it only in tests — rule 10.

use super::caveat_support::*;

/// The cap wall, both halves, and the deny is opaque. `insight.vocab.*` is ADMIN; the queue caps a
/// member holds buy neither the write nor the roster read.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_queue_caps_buy_neither_the_read_nor_the_write() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let member = principal("user:member", "nube", &caps());

    for (tool, args) in [
        ("insight.vocab.list", json!({})),
        (
            "insight.vocab.set",
            json!({ "key": "category", "values": VALUES }),
        ),
    ] {
        let err = call(&node, &member, "nube", tool, args).await.unwrap_err();
        assert!(
            matches!(err, ToolError::Denied),
            "{tool} must be Denied for a member, got {err:?}"
        );
    }
}

/// A round trip through the real dispatcher: declare, read it back, and the declaration is what the
/// raise path then enforces. The last clause is the one that matters — a door that wrote a row
/// nothing read would be decorative.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_declaration_round_trips_and_the_raise_path_enforces_it() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", &admin_caps());

    call(
        &node,
        &admin,
        "nube",
        "insight.vocab.set",
        json!({ "key": "category", "values": VALUES, "gates": [GATE] }),
    )
    .await
    .expect("declared");

    let page = call(&node, &admin, "nube", "insight.vocab.list", json!({}))
        .await
        .expect("roster");
    let vocabs = page["vocabs"].as_array().expect("vocabs array");
    assert_eq!(vocabs.len(), 1, "{page}");
    assert_eq!(vocabs[0]["key"], json!("category"));
    assert_eq!(vocabs[0]["values"], json!(VALUES));
    assert_eq!(vocabs[0]["gates"], json!([GATE]));

    // …and `raise` now refuses a value outside it, naming the set. This is the assertion that makes
    // the door real: the declaration governs.
    let raiser = principal("user:member", "nube", &caps());
    let err = call(
        &node,
        &raiser,
        "nube",
        "insight.raise",
        raise_input("vocab-door-a", "warning", 1, Some("not-declared"), &[]),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&err, ToolError::BadInput(m) if m.contains("not-declared")),
        "the refusal names the offending value: {err:?}"
    );
}

/// **A gate must be one of the values it gates.** A gating value nothing can be raised as arms a
/// caveat that never fires — and a caveat that never fires is indistinguishable from a workspace
/// with nothing to caveat, which is the failure this plane cannot afford.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_gate_outside_the_declared_values_is_refused() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", &admin_caps());

    let err = call(
        &node,
        &admin,
        "nube",
        "insight.vocab.set",
        json!({ "key": "category", "values": VALUES, "gates": ["nope"] }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&err, ToolError::BadInput(m) if m.contains("nope")),
        "{err:?}"
    );

    // REFUSED MEANS UNCHANGED. The roster is still empty — a rejected declaration must not have
    // half-written.
    let page = call(&node, &admin, "nube", "insight.vocab.list", json!({}))
        .await
        .expect("roster");
    assert_eq!(page["vocabs"].as_array().map(Vec::len), Some(0), "{page}");
}

/// The shape rules, each with its own refusal: a blank key, a `:` in the key (it is a record-id
/// segment), a blank value, and a duplicate.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_shape_rules_each_refuse_with_their_own_sentence() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", &admin_caps());

    for (args, needle) in [
        (json!({ "key": "", "values": ["a"] }), "needs a key"),
        (
            json!({ "key": "a:b", "values": ["a"] }),
            "single record-id segment",
        ),
        (
            json!({ "key": "category", "values": ["a", "  "] }),
            "may not be blank",
        ),
        (
            json!({ "key": "category", "values": ["a", "a"] }),
            "duplicate",
        ),
    ] {
        let err = call(&node, &admin, "nube", "insight.vocab.set", args.clone())
            .await
            .unwrap_err();
        assert!(
            matches!(&err, ToolError::BadInput(m) if m.contains(needle)),
            "{args} should refuse with {needle:?}, got {err:?}"
        );
    }
}

/// **Declaring nothing RE-OPENS the key** — lb's own rule is that an undeclared vocabulary
/// validates nothing, so an empty `values` list and no row at all are the same statement. There is
/// deliberately no delete verb to disagree with this one.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_empty_declaration_re_opens_the_key() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", "nube", &admin_caps());
    let raiser = principal("user:member", "nube", &caps());

    call(
        &node,
        &admin,
        "nube",
        "insight.vocab.set",
        json!({ "key": "category", "values": VALUES }),
    )
    .await
    .expect("declared");
    call(
        &node,
        &raiser,
        "nube",
        "insight.raise",
        raise_input("vocab-open-a", "warning", 1, Some("anything"), &[]),
    )
    .await
    .expect_err("closed while declared");

    call(
        &node,
        &admin,
        "nube",
        "insight.vocab.set",
        json!({ "key": "category", "values": [] }),
    )
    .await
    .expect("re-opened");
    call(
        &node,
        &raiser,
        "nube",
        "insight.raise",
        raise_input("vocab-open-b", "warning", 2, Some("anything"), &[]),
    )
    .await
    .expect("an undeclared vocabulary validates nothing");
}

/// The workspace wall: a declaration in one workspace is invisible in another, and does not govern
/// it. Structural (the store scan is ws-scoped) but asserted, because "structural" is a claim.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_declaration_does_not_cross_the_workspace_wall() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin_a = principal("user:admin", "nube", &admin_caps());
    let admin_b = principal("user:admin", "other", &admin_caps());

    call(
        &node,
        &admin_a,
        "nube",
        "insight.vocab.set",
        json!({ "key": "category", "values": VALUES }),
    )
    .await
    .expect("declared in nube");

    let page = call(&node, &admin_b, "other", "insight.vocab.list", json!({}))
        .await
        .expect("roster in other");
    assert_eq!(
        page["vocabs"].as_array().map(Vec::len),
        Some(0),
        "ws `other` sees none of nube's declarations: {page}"
    );

    // …and ws `other` is therefore still OPEN: nube's closed set does not govern it.
    let raiser = principal("user:member", "other", &caps());
    call(
        &node,
        &raiser,
        "other",
        "insight.raise",
        raise_input("vocab-wall-a", "warning", 1, Some("anything"), &[]),
    )
    .await
    .expect("other is unseeded and therefore open");
}
