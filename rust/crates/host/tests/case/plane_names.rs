//! `case.names` — the display names of the people a caller's cases mention, and the wall around it.
//!
//! Part of the `plane` suite (see `plane_support.rs` for the fixtures and the full preamble). One
//! binary: `case_suite.rs`.
//!
//! The verb names a subject only when it is a live member of the caller's workspace with a display
//! name set. These tests hold both halves: a member's name comes back to a viewer-grade caller, and
//! a subject from outside the workspace (whatever the node's identity table holds) answers nothing.

use lb_authz::{identity_create, identity_set_email};

use super::plane_support::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_viewer_reads_the_names_of_members_and_nothing_else() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    identity_create(&node.store, "user:priya", Some("Priya Sharma"), 1)
        .await
        .expect("priya named");
    identity_set_email(&node.store, "user:priya", "Priya@ESR.example")
        .await
        .expect("priya's email");
    // A person the node knows, but who is not in `nube`.
    identity_create(&node.store, "user:outsider", Some("Out Sider"), 1)
        .await
        .expect("outsider named");

    let lister = principal("user:test", "nube", &[LIST]);
    let out = call(
        &node,
        &lister,
        "nube",
        "case.names",
        json!({ "subjects": ["user:priya", "user:outsider", "user:test", "team:mechanical", "user:priya"] }),
    )
    .await
    .expect("names are viewer-grade, through the `case.list` alias");

    assert_eq!(
        out,
        json!([{ "sub": "user:priya", "name": "Priya Sharma", "email": "priya@esr.example" }]),
        "only a named member of this workspace comes back, once: {out}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn more_subjects_than_one_screen_is_refused() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let lister = principal("user:test", "nube", &[LIST]);
    let many: Vec<String> = (0..201).map(|i| format!("user:u{i}")).collect();
    let err = call(
        &node,
        &lister,
        "nube",
        "case.names",
        json!({ "subjects": many }),
    )
    .await
    .expect_err("a call past the bound is refused");
    assert!(format!("{err:?}").contains("at most 200"), "{err:?}");
}
