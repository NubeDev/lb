//! (e) The caveat stamp itself: the subject intersection, the negative case, the clear on re-raise, and the silenced notify ladder.
//!
//! Part of the `caveat` suite (see `caveat_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::caveat_support::*;

// --- (e) the caveat stamp ------------------------------------------------------------------------

/// **THE STAMP.** An open finding in the workspace's declared gating category, on `point:X`. A
/// second finding derived from `point:X` must carry its id — on the outcome, on `get`, and on the
/// roster page (`list`), because greying a soft row is a roster job.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_open_gating_finding_caveats_a_finding_on_the_same_subjects() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_vocab(&node, ws).await;
    let p = principal("user:test", ws, &caps());

    let dq = raise(
        &node,
        &p,
        ws,
        raise_input("sensor-stuck", "warning", 1, Some(GATE), &["point:X"]),
    )
    .await
    .expect("dq raise")["id"]
        .as_str()
        .unwrap()
        .to_string();

    let out = raise(
        &node,
        &p,
        ws,
        raise_input(
            "intensity-high",
            "critical",
            2,
            Some(VALUES[2]),
            &["point:X"],
        ),
    )
    .await
    .expect("finding raise");
    assert_eq!(out["caveated"], true, "the outcome says so: {out}");

    let id = out["id"].as_str().unwrap();
    let got = call(&node, &p, ws, "insight.get", json!({ "id": id }))
        .await
        .expect("get ok");
    assert_eq!(got["caveats"], json!([dq]), "get echoes the caveat: {got}");

    let page = call(&node, &p, ws, "insight.list", json!({}))
        .await
        .expect("list ok");
    let row = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == id)
        .expect("row present");
    assert_eq!(row["caveats"], json!([dq]), "list echoes it too: {row}");

    // The gating finding is NOT caveated by itself, nor by another gating finding — a data-quality
    // finding is not softened by data quality, and mutual caveating would silence both.
    let dq_row = call(&node, &p, ws, "insight.get", json!({ "id": &dq }))
        .await
        .expect("get dq");
    assert!(
        dq_row["caveats"]
            .as_array()
            .map(|a| a.is_empty())
            .unwrap_or(true),
        "the gating finding stays uncaveated: {dq_row}"
    );
}

/// No subject overlap ⇒ no caveat. The join is on `evidence.subjects`, not on "there is a data
/// quality problem somewhere in this workspace".
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_finding_on_different_subjects_is_not_caveated() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_vocab(&node, ws).await;
    let p = principal("user:test", ws, &caps());

    raise(
        &node,
        &p,
        ws,
        raise_input("sensor-stuck", "warning", 1, Some(GATE), &["point:X"]),
    )
    .await
    .expect("dq raise");
    let out = raise(
        &node,
        &p,
        ws,
        raise_input("other", "critical", 2, Some(VALUES[2]), &["point:Y"]),
    )
    .await
    .expect("raise");
    assert_eq!(out["caveated"], false, "different points, no caveat: {out}");
}

/// **Self-healing.** Resolve the gating finding and the next raise of the dependent one comes back
/// clean. The caveat is a statement about the world right now, so it is REFRESHED (not merged) on
/// every raise — a permanent caveat is the failure mode that teaches operators to ignore it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn resolving_the_gating_finding_clears_the_caveat_on_the_next_raise() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_vocab(&node, ws).await;
    let p = principal("user:test", ws, &caps());

    let dq = raise(
        &node,
        &p,
        ws,
        raise_input("sensor-stuck", "warning", 1, Some(GATE), &["point:X"]),
    )
    .await
    .expect("dq raise")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let id = raise(
        &node,
        &p,
        ws,
        raise_input("dependent", "critical", 2, Some(VALUES[2]), &["point:X"]),
    )
    .await
    .expect("raise")["id"]
        .as_str()
        .unwrap()
        .to_string();

    call(
        &node,
        &p,
        ws,
        "insight.resolve",
        json!({ "id": &dq, "ts": 3 }),
    )
    .await
    .expect("resolved");

    let out = raise(
        &node,
        &p,
        ws,
        raise_input("dependent", "critical", 4, Some(VALUES[2]), &["point:X"]),
    )
    .await
    .expect("re-raise");
    assert_eq!(out["caveated"], false, "the caveat cleared: {out}");
    let got = call(&node, &p, ws, "insight.get", json!({ "id": &id }))
        .await
        .expect("get ok");
    assert!(
        got["caveats"]
            .as_array()
            .map(|a| a.is_empty())
            .unwrap_or(true),
        "the stored list was REFRESHED, not merged: {got}"
    );
}

/// **THE DELIVERY ASSERTION.** A `critical` first-ever raise is the loudest thing the notify path
/// can produce. Caveated, it must post NOTHING; the identical uncaveated raise on a sibling key
/// must post. Asserted through a real subscription and the real delivered inbox — the control half
/// is what stops this passing because the machine happened to be silent.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_caveated_critical_raise_delivers_nothing_while_an_uncaveated_one_does() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_vocab(&node, ws).await;
    let p = principal("user:test", ws, &caps());

    // One subscription over everything, into a real channel.
    call(
        &node,
        &p,
        ws,
        "insight.sub.create",
        json!({ "sink": { "kind": "channel", "channel": "ops" }, "filter": {}, "now": 1 }),
    )
    .await
    .expect("sub created");

    raise(
        &node,
        &p,
        ws,
        raise_input("sensor-stuck", "warning", 10, Some(GATE), &["point:X"]),
    )
    .await
    .expect("dq raise");

    // Caveated (subjects overlap the open gating finding) …
    raise(
        &node,
        &p,
        ws,
        raise_input(
            "caveated-key",
            "critical",
            20,
            Some(VALUES[2]),
            &["point:X"],
        ),
    )
    .await
    .expect("caveated raise");
    // … and the control, identical but for its subjects.
    raise(
        &node,
        &p,
        ws,
        raise_input("clean-key", "critical", 30, Some(VALUES[2]), &["point:Y"]),
    )
    .await
    .expect("clean raise");

    let items = lb_host::list_inbox(&node.store, &p, ws, "ops")
        .await
        .expect("inbox readable");
    let posts = |key: &str| items.iter().filter(|i| i.body.contains(key)).count();

    assert_eq!(
        posts("clean-key"),
        1,
        "the CONTROL must deliver, or this test proves nothing: {:?}",
        items.iter().map(|i| &i.body).collect::<Vec<_>>()
    );
    assert_eq!(
        posts("caveated-key"),
        0,
        "a caveated critical raise must not break through and must not post immediately: {:?}",
        items.iter().map(|i| &i.body).collect::<Vec<_>>()
    );
}
