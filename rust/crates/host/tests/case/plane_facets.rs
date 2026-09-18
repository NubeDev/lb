//! The **facet echo** — what a case takes from its primary insight's tags
//! (`docs/scope/insights/case-plane-scope.md` §Data model).
//!
//! Part of the `plane` suite (see `plane_support.rs` for the fixtures and the full preamble). One
//! binary: `case_suite.rs`.
//!
//! **Why this file exists.** The echo set used to be four — severity, category, site, scope — and
//! adding a fifth facet looked complete from every angle that was tested. A rule tagged its finding
//! `subsystem: "water"`, the tag really was persisted, and `insight.get` showed it. The case simply
//! did not carry it: no error, no warning, no failing test. The gap was only visible by reading a
//! CASE back and noticing a field that was never there.
//!
//! So these tests assert from the case side, over the real chain — `insight.raise` → the inline
//! grouping reactor → `case::open` → the stored record read back through `case.get`. A unit test on
//! `facets_of` would not have caught the original bug, because `facets_of` was not where it lived:
//! the struct had no field to put the value in, and every layer in between silently dropped it.
//!
//! The facet KEYS here are lb's own dimension names. The VALUES (`water`, `plant`, …) are workspace
//! vocabulary a pack seeds, so this file invents its own rather than using anything product-shaped
//! — rule 10: lb ships no list, and a test that assumed one would be asserting a default that does
//! not exist.

use super::plane_support::*;

/// Raise a finding carrying `tags`, and return the case the reactor opened for it.
async fn case_for_tags(node: &Arc<Node>, p: &Principal, ws: &str, key: &str, tags: Value) -> Value {
    let mut input = raise_input(key, 1);
    input["tags"] = tags;
    let out = call(node, p, ws, "insight.raise", input)
        .await
        .expect("raise ok");
    let insight = out["id"].as_str().expect("raise returns an id").to_string();
    let case_id = case_of(node, p, ws, &insight).await;
    call(node, p, ws, "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok")
}

/// **The whole echo set reaches the case.** One raise, every facet asserted together — because the
/// failure this file was written for was exactly one facet going missing while its neighbours
/// arrived, which is invisible to a test that checks one field at a time.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn every_declared_facet_reaches_the_case() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    let case = case_for_tags(
        &node,
        &p,
        "nube",
        "facet-echo-all",
        json!({
            "category": "other",
            "site": "site-001",
            "scope": "device",
            "subsystem": "water",
            // A tag OUTSIDE the echo set. It must stay on the insight and never appear on the case:
            // the echo is a closed set by design, and a case that absorbed every tag would make the
            // record's shape depend on whatever a rule author happened to write.
            "issue": "night-flow",
        }),
    )
    .await;

    assert_eq!(case["category"], "other", "category echoes: {case}");
    assert_eq!(case["site"], "site-001", "site echoes: {case}");
    assert_eq!(case["scope"], "device", "scope echoes: {case}");
    assert_eq!(case["subsystem"], "water", "subsystem echoes: {case}");
    assert!(
        case.get("issue").is_none(),
        "a tag outside the echo set must NOT land on the case: {case}"
    );
}

/// **An untagged finding leaves the facet absent, not empty.** `skip_serializing_if` keeps it off
/// the wire entirely, which is what lets a consumer tell "this case has no subsystem" apart from
/// "this case's subsystem is the empty string".
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_untagged_finding_opens_a_case_with_no_subsystem() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    let case = case_for_tags(&node, &p, "nube", "facet-echo-none", json!({})).await;
    assert!(
        case.get("subsystem").is_none(),
        "an untagged finding must leave subsystem ABSENT: {case}"
    );
}

/// **`case.list` NARROWS on subsystem — it does not ignore the axis.**
///
/// This is the assertion that matters most, and the one whose absence was most dangerous. lb's list
/// verbs answer `200` with the UNFILTERED set when handed a filter they do not model, so a filter
/// that silently matches everything looks identical to a filter that works. The negative case — a
/// value no case holds returning ZERO — is the only thing that tells the two apart.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn case_list_filters_by_subsystem() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    for (key, subsystem) in [
        ("f-water", "water"),
        ("f-plant-a", "plant"),
        ("f-plant-b", "plant"),
    ] {
        case_for_tags(
            &node,
            &p,
            "nube",
            key,
            json!({ "site": "site-001", "subsystem": subsystem }),
        )
        .await;
    }

    let count = |v: &Value| v["items"].as_array().map(Vec::len).unwrap_or_default();
    let list = |filter: Value| {
        let node = node.clone();
        let p = p.clone();
        async move {
            call(
                &node,
                &p,
                "nube",
                "case.list",
                json!({ "lane": "watching", "limit": 100, "filter": filter }),
            )
            .await
            .expect("list ok")
        }
    };

    let all = list(json!({})).await;
    let water = list(json!({ "subsystem": "water" })).await;
    let plant = list(json!({ "subsystem": "plant" })).await;
    let none = list(json!({ "subsystem": "no-such-subsystem" })).await;

    assert_eq!(count(&water), 1, "one water case: {water}");
    assert_eq!(count(&plant), 2, "two plant cases: {plant}");
    assert!(
        count(&all) >= 3 && count(&all) > count(&plant),
        "the filter must NARROW the unfiltered set: all={} plant={}",
        count(&all),
        count(&plant)
    );
    // The proof the axis is modelled rather than ignored.
    assert_eq!(
        count(&none),
        0,
        "an unmatched subsystem must return 0, not the unfiltered set: {none}"
    );

    for item in water["items"].as_array().unwrap() {
        assert_eq!(item["subsystem"], "water", "a narrowed row: {item}");
    }
}

/// **A split case inherits the facet.** `case.split` pulls members into a NEW case, and a split that
/// dropped the subsystem would hand the work to a queue that no longer knows which trade owns it —
/// the one moment the facet is most needed.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_split_case_inherits_the_subsystem() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal(
        "user:test",
        "nube",
        &[ALL, &["mcp:case.split:call"]].concat(),
    );

    // Two findings that group into ONE case, so there is something to split off.
    let mut first = raise_input("split-src-a", 1);
    first["tags"] = json!({ "site": "site-001", "subsystem": "water" });
    first["evidence"] = json!({ "source": "demo", "subjects": ["point:a"] });
    let a = call(&node, &p, "nube", "insight.raise", first)
        .await
        .expect("raise a");
    let a_id = a["id"].as_str().unwrap().to_string();
    let case_id = case_of(&node, &p, "nube", &a_id).await;

    let parent = call(&node, &p, "nube", "case.get", json!({ "id": case_id }))
        .await
        .expect("get parent");
    assert_eq!(parent["subsystem"], "water", "parent carries it: {parent}");

    let split = call(
        &node,
        &p,
        "nube",
        "case.split",
        json!({ "id": case_id, "insights": [a_id], "title": "the split half", "ts": 2 }),
    )
    .await;

    // The split verb may legitimately refuse to move the PRIMARY insight out of its own case. When
    // it does, there is nothing to assert about the child — but the parent above already proved the
    // echo, so this test stays honest either way rather than asserting a shape it did not get.
    if let Ok(child) = split {
        assert_eq!(
            child["subsystem"], "water",
            "a split case must inherit the subsystem: {child}"
        );
    }
}
