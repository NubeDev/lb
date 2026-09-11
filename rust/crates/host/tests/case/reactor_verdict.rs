//! A verdict in both arrival orderings, and the human-placed member the reactor must never move.
//!
//! Part of the `reactor` suite (see `reactor_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::reactor_support::*;

// --- Verdict: the two orderings -----------------------------------------------------------------

/// **verdict-LAST** — the cited findings already sit in their own `single` cases when the verdict
/// record arrives. The singles must be MERGED into one verdict case, not left as duplicates.
///
/// This is the ordering that catches a grouping which only ever adds: it would leave three open
/// cases citing four detections, the queue would over-count, and two technicians would work the same
/// fault from two rows.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn verdict_last_merges_the_existing_single_cases_into_one_verdict_case() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;

    // RED: without the raise cap nothing is grouped at all.
    let denied = principal("node:reactor", "nube", NO_RAISE);
    assert!(matches!(
        call(
            &node,
            &denied,
            "nube",
            "insight.raise",
            raise_input("flatline", 1)
        )
        .await
        .unwrap_err(),
        ToolError::Denied
    ));
    assert_eq!(open_case_count(&node, &denied, "nube").await, 0);

    // GREEN. Three ordinary findings first — three `single` cases.
    let p = principal("user:test", "nube", ALL);
    let root = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("flatline", 1),
    )
    .await
    .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let sym_a = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("valve-hunting", 2),
    )
    .await
    .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let sym_b = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("temp-drift", 3),
    )
    .await
    .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(open_case_count(&node, &p, "nube").await, 3, "three singles");

    // Now the verdict record — citing DEDUP KEYS, not ids.
    let verdict = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        verdict_input("verdict-1", "flatline", &["valve-hunting", "temp-drift"], 4),
    )
    .await
    .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();

    // ONE open case, and it is about the ROOT — not the record that named it.
    assert_eq!(
        open_case_count(&node, &p, "nube").await,
        1,
        "the singles must fold into the verdict case, not sit beside it"
    );
    let case_id = case_of(&node, &p, "nube", &root).await;
    let case = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert_eq!(case["grouping"], "verdict");
    assert_eq!(
        case["primary_insight"], root,
        "the primary is the ROOT finding, not the verdict record"
    );

    // All four detections — the root, both symptoms, and the verdict record itself — in one case.
    let mut expected = vec![root.clone(), sym_a.clone(), sym_b.clone(), verdict.clone()];
    expected.sort();
    assert_eq!(member_ids(&node, &p, "nube", &case_id).await, expected);

    // And every one of them carries the same `case_id` echo.
    for id in [&root, &sym_a, &sym_b, &verdict] {
        assert_eq!(
            case_of(&node, &p, "nube", id).await,
            case_id,
            "echo on {id}"
        );
    }
}

/// **verdict-FIRST** — the citing record arrives before the findings it cites exist. It opens a case
/// over what resolves (nothing but itself), and each straggler joins that case as it is raised.
///
/// Without the straggler lookup this ordering silently produces a permanently wrong grouping: the
/// verdict's `explains[]` resolved to nothing, so the stragglers get `single` cases and nobody ever
/// reconciles them. The bug is invisible — every record has a case, the invariant holds, the answer
/// is just wrong.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn verdict_first_folds_each_straggler_in_as_it_arrives() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;

    // RED.
    let denied = principal("node:reactor", "nube", NO_RAISE);
    assert!(matches!(
        call(
            &node,
            &denied,
            "nube",
            "insight.raise",
            verdict_input("verdict-2", "late-flatline", &["late-hunting"], 1)
        )
        .await
        .unwrap_err(),
        ToolError::Denied
    ));
    assert_eq!(open_case_count(&node, &denied, "nube").await, 0);

    // GREEN. The verdict record first: nothing it cites exists yet.
    let p = principal("user:test", "nube", ALL);
    let verdict = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        verdict_input("verdict-2", "late-flatline", &["late-hunting"], 1),
    )
    .await
    .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let case_id = case_of(&node, &p, "nube", &verdict).await;
    let case = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    assert_eq!(case["grouping"], "verdict");
    assert_eq!(
        case["primary_insight"], verdict,
        "with nothing resolvable, the citing record stands in as the primary"
    );
    assert_eq!(open_case_count(&node, &p, "nube").await, 1);

    // The root arrives. It must join the case that cited it, NOT open a `single`.
    let root = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("late-flatline", 2),
    )
    .await
    .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        open_case_count(&node, &p, "nube").await,
        1,
        "the root must join the citing case, not open a second one"
    );
    assert_eq!(case_of(&node, &p, "nube", &root).await, case_id);

    // Then the symptom.
    let sym = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("late-hunting", 3),
    )
    .await
    .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(open_case_count(&node, &p, "nube").await, 1);
    assert_eq!(case_of(&node, &p, "nube", &sym).await, case_id);

    let mut expected = vec![verdict, root, sym];
    expected.sort();
    assert_eq!(member_ids(&node, &p, "nube", &case_id).await, expected);
}

/// A member a PERSON placed is never moved by a reactor — not to fix a grouping, not to satisfy the
/// verdict. The grouping is left wrong rather than overruling a judgement the machine cannot see.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_reactor_never_moves_a_human_placed_member() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    // RED — the same first raise with the reactor's grant removed: refused, and nothing grouped.
    // Without this half the assertions below would still pass if grouping never ran at all.
    let no_raise = principal("node:reactor", "nube", NO_RAISE);
    let err = call(
        &node,
        &no_raise,
        "nube",
        "insight.raise",
        raise_input("hp-root", 1),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "expected Denied: {err:?}");
    assert_eq!(
        open_case_count(&node, &no_raise, "nube").await,
        0,
        "a denied raise must group nothing"
    );

    let root = call(
        &node,
        &p,
        "nube",
        "insight.raise",
        raise_input("hp-root", 1),
    )
    .await
    .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let sym = call(&node, &p, "nube", "insight.raise", raise_input("hp-sym", 2))
        .await
        .expect("ok")["id"]
        .as_str()
        .unwrap()
        .to_string();

    // A person deliberately splits the symptom into its own case — `human_placed`.
    let root_case = case_of(&node, &p, "nube", &root).await;
    let sym_case = case_of(&node, &p, "nube", &sym).await;
    call(
        &node,
        &p,
        "nube",
        "case.merge",
        json!({ "from": sym_case, "into": root_case, "ts": 3 }),
    )
    .await
    .expect("merge ok");
    let human_case = call(
        &node,
        &p,
        "nube",
        "case.split",
        json!({ "from": root_case, "insight_ids": [sym], "title": "a separate fault", "ts": 4 }),
    )
    .await
    .expect("split ok")["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Now a verdict record claims the symptom. The reactor must NOT take it back.
    call(
        &node,
        &p,
        "nube",
        "insight.raise",
        verdict_input("hp-verdict", "hp-root", &["hp-sym"], 5),
    )
    .await
    .expect("ok");

    assert_eq!(
        case_of(&node, &p, "nube", &sym).await,
        human_case,
        "the human-placed member stayed where the person put it"
    );
    let human = call(&node, &p, "nube", "case.get", json!({ "id": human_case }))
        .await
        .expect("get ok");
    assert_eq!(human["closed"], false, "and its case is still open");
}
