//! The **money echo** — a case takes its `impact_rate` from its primary insight's stated impact,
//! and only when that impact was stated in money per day
//! (`docs/scope/insights/case-money-producer-scope.md`).
//!
//! Part of the `plane` suite (see `plane_support.rs` for the fixtures and the full preamble). One
//! binary: `case_suite.rs`.
//!
//! These run the REAL path: `insight.raise` → the inline grouping reactor → `case::open` → the
//! stored record read back through `case.get`. Nothing here constructs a `Case` or calls
//! `impact_of` directly — `case::impact`'s own unit tests cover the parse table, and these cover
//! the thing a unit test cannot, which is that the number survives the whole chain onto a record an
//! operator's queue reads.

use super::plane_support::*;

/// A raise carrying `analysis.estimated_impact` and, optionally, evidence subjects (the hook a
/// data-quality finding uses to caveat this one).
fn raise_with_impact(dedup_key: &str, ts: u64, impact: Value, subjects: &[&str]) -> Value {
    let mut input = raise_input(dedup_key, ts);
    input["analysis"] = json!({ "estimated_impact": impact });
    if !subjects.is_empty() {
        input["evidence"] = json!({ "source": "demo", "subjects": subjects });
    }
    input
}

/// The case the reactor opened for a raise, read back through `case.get`.
async fn case_for(node: &Arc<Node>, p: &Principal, ws: &str, input: Value) -> Value {
    let out = call(node, p, ws, "insight.raise", input)
        .await
        .expect("raise ok");
    let insight = out["id"].as_str().expect("raise returns an id").to_string();
    let case_id = case_of(node, p, ws, &insight).await;
    call(node, p, ws, "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok")
}

/// **The producer works.** A rule that prices its finding in `AUD/day` opens a case carrying that
/// rate, tiered `claimed` — the join this whole scope exists to make.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_priced_finding_opens_a_case_carrying_its_rate() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    let case = case_for(
        &node,
        &p,
        "nube",
        raise_with_impact(
            "priced",
            1,
            json!({ "value": 180.0, "unit": "AUD/day", "note": "vs 1.8 kL baseline" }),
            &[],
        ),
    )
    .await;

    assert_eq!(
        case["impact_rate"].as_f64(),
        Some(180.0),
        "the rate did not reach the case: {case}"
    );
    assert_eq!(case["impact_tier"].as_str(), Some("claimed"));
}

/// **The unit is checked, not assumed.** A `sigma` deviation is a real number in a real unit and it
/// is NOT money — copying it would put `$3.20/day` in front of a customer. The case opens untiered,
/// which renders as `—`: honest, and visibly nothing rather than invisibly wrong.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_impact_in_an_unlike_unit_reaches_no_case() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    for (key, unit) in [("sigma", "sigma"), ("kl", "kL"), ("weekly", "AUD/week")] {
        let case = case_for(
            &node,
            &p,
            "nube",
            raise_with_impact(key, 1, json!({ "value": 3.2, "unit": unit }), &[]),
        )
        .await;
        assert!(
            case["impact_rate"].is_null(),
            "unit {unit:?} leaked onto the case as money: {case}"
        );
        assert!(case["impact_tier"].is_null(), "unit {unit:?}: {case}");
    }
}

/// **A producer that cannot price its finding says so, and the case stays untiered.** The honest
/// `"N/A (data quality)"` the `Quantity` doc defends: a note with no value is not a refusal, it is
/// the type working, and it must not become a zero.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_note_only_impact_leaves_the_case_untiered() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    let case = case_for(
        &node,
        &p,
        "nube",
        raise_with_impact(
            "unpriceable",
            1,
            json!({ "note": "N/A (data quality)" }),
            &[],
        ),
    )
    .await;

    assert!(case["impact_rate"].is_null(), "{case}");
    assert!(case["impact_tier"].is_null(), "{case}");
}

/// **`withheld` wins — the rule most likely to be lost the day a producer finally exists.**
///
/// A finding resting on a point another OPEN data-quality finding says is broken is caveated, and a
/// caveated case shows no money however confident its rate is. The rate is still STORED (it becomes
/// showable the moment the caveat clears); the tier is what suppresses it, and every consumer's
/// `withheld` branch turns that into no figure.
///
/// Both halves are asserted in one test on purpose: `impact_rate: 420` with `impact_tier: withheld`
/// is the exact pair that a change deleting the withhold would turn into a rendered $420/day, and a
/// test that only checked the tier would still pass if the rate had gone missing instead.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_caveated_case_withholds_a_rate_it_really_holds() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    seed_gating_vocab(&node, "nube").await;
    let p = principal("user:test", "nube", ALL);

    // The gating finding first: an open data-quality insight on the point the priced finding rests
    // on. lb stamps the caveat by intersecting `evidence.subjects`, so the two must name it.
    call(&node, &p, "nube", "insight.raise", {
        let mut dq = raise_input("broken-sensor", 1);
        dq["tags"] = json!({ "category": GATING_CATEGORY });
        dq["evidence"] = json!({ "source": "demo", "subjects": ["point:p1"] });
        dq
    })
    .await
    .expect("the gating raise lands");

    let case = case_for(
        &node,
        &p,
        "nube",
        raise_with_impact(
            "priced-but-caveated",
            2,
            json!({ "value": 420.0, "unit": "AUD/day" }),
            &["point:p1"],
        ),
    )
    .await;

    assert!(
        case["caveated"].as_bool().unwrap_or(false),
        "the fixture did not produce a caveated case, so this asserts nothing: {case}"
    );
    assert_eq!(
        case["impact_rate"].as_f64(),
        Some(420.0),
        "the rate must be KEPT — it becomes showable when the caveat clears: {case}"
    );
    assert_eq!(
        case["impact_tier"].as_str(),
        Some("withheld"),
        "a caveated case must withhold its figure: {case}"
    );
}
