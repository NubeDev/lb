//! The tag **fold** — turning the insight's multi-source tag edges into the flat `{k: v}` echo,
//! with `Human > Producer > Inferred > System` precedence
//! (`docs/scope/insights/insight-tag-precedence-scope.md`).
//!
//! ## The bug this file exists to kill
//!
//! Tag edge identity in the graph is `(entity, tag, source)`, so two edges for one key legitimately
//! coexist — the nightly rule asserted `classification=plumbing` as `Producer`, an operator
//! corrected it to `mechanical` as `Human`. `Insight.tags` is a flat map, so the echo must pick
//! one. Before this file it did not pick: it collected into a `BTreeMap` and kept whichever edge
//! `tags.of` happened to return last. That order is unspecified, so the rendered dimension column
//! for a corrected key was **non-deterministic across raises** — and worse, the machine re-asserts
//! on every firing, so last-write-wins means the operator's correction silently reverts overnight.
//!
//! That is the same class of trust bug as evicting a human comment, and it is harder to see:
//! nothing errors, the column just reads differently tomorrow.
//!
//! ## The rule
//!
//! For one key: **highest-precedence source wins** (`Human` > `Producer` > `Inferred` > `System`);
//! within one source, the **newest `Provenance.at`** wins; and a remaining tie breaks on the value
//! string, so insertion order can never change the answer. The last clause is not decoration — it
//! is what makes the fold a total order and therefore deterministic, which is half the scope's
//! testing plan.
//!
//! *Rejected: newest-wins regardless of source* — simpler, and wrong in exactly the case that
//! matters, since the producer re-asserts on every firing and so always eventually wins. A rule
//! that guarantees the human loses is not a tie-break.
//!
//! One responsibility: the precedence fold + the graph read that feeds it.

use std::collections::BTreeMap;

use lb_store::Store;
use lb_tags::{Applied, Source};

/// Precedence rank of a tag source — higher wins. The ONE place the order is stated.
///
/// `Human` above everything because a person deliberately recorded it; `Producer` above `Inferred`
/// because an assertion beats a guess; `System` lowest because it is bookkeeping the other three
/// are all more specific than.
fn rank(source: Source) -> u8 {
    match source {
        Source::Human => 3,
        Source::Producer => 2,
        Source::Inferred => 1,
        Source::System => 0,
    }
}

/// Stringify a tag value for the flat echo. A bare JSON string is its own contents (so `"eu"` is
/// `eu`, not `"eu"`); anything else keeps its JSON rendering, which is at least round-trippable.
fn value_of(value: serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s,
        other => other.to_string(),
    }
}

/// Fold every applied edge into one `{key: value}` map under the precedence rule (module doc).
///
/// **Pure and order-independent**: the winner for a key is the maximum of `(rank, at, value)` over
/// its edges, and a maximum does not depend on the order the candidates arrive in. That is the
/// property the determinism test asserts by reversing the input.
pub(super) fn fold_facets(applied: Vec<Applied>) -> BTreeMap<String, String> {
    // Per key, the winning (rank, at, value) so far. `value` is in the key so ties are total.
    let mut best: BTreeMap<String, (u8, u64, String)> = BTreeMap::new();
    for edge in applied {
        let candidate = (rank(edge.source), edge.at, value_of(edge.value));
        match best.get_mut(&edge.key) {
            Some(current) => {
                if candidate > *current {
                    *current = candidate;
                }
            }
            None => {
                best.insert(edge.key, candidate);
            }
        }
    }
    best.into_iter().map(|(k, (_, _, v))| (k, v)).collect()
}

/// Materialize the insight's tag facets as `{k: v}` — the matcher's subset check AND the record's
/// tag echo (`insight-tag-echo-scope.md`), folded by precedence (`insight-tag-precedence-scope.md`).
/// On any graph error, falls back to this raise's declared tags so a hiccup cannot silently drop a
/// match or blank a dimension column.
///
/// Reads the graph RAW (`lb_tags::of`, not the `mcp:tags.of:call`-gated host verb) for the same
/// reason `insight_list` resolves its facet filter raw: `mcp:insight.raise:call` already authorized
/// this workspace's insight write, and reading back the tags of the entity this very call just
/// created is not a second privilege. Gating it on `tags.of` would mean a producer without tag caps
/// silently gets an echo built from its own declaration instead of the union — the exact bug the
/// scope exists to prevent, in the failure mode hardest to see.
pub(super) async fn materialize_facets(
    store: &Store,
    ws: &str,
    entity: &str,
    fallback: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    match lb_tags::of(store, ws, entity).await {
        Ok(applied) if !applied.is_empty() => fold_facets(applied),
        _ => fallback.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// One applied edge. `confidence`/`expires` are irrelevant to the fold and stay at their
    /// hard-assertion defaults so the test states only what it is about.
    fn edge(key: &str, value: &str, source: Source, at: u64) -> Applied {
        Applied {
            key: key.into(),
            value: json!(value),
            at,
            by: "user:test".into(),
            source,
            confidence: 1.0,
            expires: None,
        }
    }

    /// **THE REGRESSION (case 1).** A producer and a human edge for one key: the human value wins.
    /// Red before the fold existed — the old code collected into a map and kept whichever edge came
    /// last, which for this input is the producer.
    #[test]
    fn a_human_correction_beats_the_producers_assertion() {
        let folded = fold_facets(vec![
            edge("classification", "mechanical", Source::Human, 10),
            edge("classification", "plumbing", Source::Producer, 20),
        ]);
        assert_eq!(
            folded.get("classification").map(String::as_str),
            Some("mechanical"),
            "the human wins even though the producer asserted LATER — the machine re-asserts on \
             every firing, so newest-wins guarantees the human always loses"
        );
    }

    /// **Case 2.** Re-running the fold on the same edges does not flip the answer — the property
    /// that makes "the correction survives tonight's raise" true, stated at the unit level.
    #[test]
    fn a_second_fold_over_the_same_edges_is_stable() {
        let edges = vec![
            edge("classification", "mechanical", Source::Human, 10),
            edge("classification", "plumbing", Source::Producer, 20),
        ];
        let first = fold_facets(edges.clone());
        let second = fold_facets(edges);
        assert_eq!(first, second);
        assert_eq!(first.get("classification").map(String::as_str), Some("mechanical"));
    }

    /// **Case 3 — determinism.** The identical edges in the reverse order give the identical map.
    /// This is the one the old collect-into-a-map code could never pass.
    #[test]
    fn reversed_insertion_order_gives_the_identical_map() {
        let mut edges = vec![
            edge("classification", "mechanical", Source::Human, 10),
            edge("classification", "plumbing", Source::Producer, 20),
            edge("building", "north", Source::Producer, 5),
            edge("building", "south", Source::Inferred, 99),
        ];
        let forward = fold_facets(edges.clone());
        edges.reverse();
        let backward = fold_facets(edges);
        assert_eq!(forward, backward);
        assert_eq!(forward.get("building").map(String::as_str), Some("north"));
    }

    /// **Case 4 — tie within one source.** Two human edges for one key: the newest `at` wins.
    #[test]
    fn within_one_source_the_newest_provenance_wins() {
        let folded = fold_facets(vec![
            edge("priority", "medium", Source::Human, 10),
            edge("priority", "high", Source::Human, 20),
        ]);
        assert_eq!(folded.get("priority").map(String::as_str), Some("high"));

        // …and the same two in the other order, because "newest" must not mean "last seen".
        let folded = fold_facets(vec![
            edge("priority", "high", Source::Human, 20),
            edge("priority", "medium", Source::Human, 10),
        ]);
        assert_eq!(folded.get("priority").map(String::as_str), Some("high"));
    }

    /// The overwhelmingly common case must be untouched: one source, one edge per key.
    #[test]
    fn a_single_source_key_folds_to_exactly_what_it_says() {
        let folded = fold_facets(vec![
            edge("building", "chullora-dc", Source::Producer, 1),
            edge("asset_type", "water-meter", Source::Producer, 1),
        ]);
        assert_eq!(folded.len(), 2);
        assert_eq!(folded["building"], "chullora-dc");
        assert_eq!(folded["asset_type"], "water-meter");
    }

    /// The last resort: same source, same `at`, two values. Something must win, and it must be the
    /// SAME something every time — otherwise the column still flickers, just more rarely.
    #[test]
    fn an_exact_tie_breaks_deterministically_on_the_value() {
        let a = fold_facets(vec![
            edge("k", "alpha", Source::Human, 7),
            edge("k", "beta", Source::Human, 7),
        ]);
        let b = fold_facets(vec![
            edge("k", "beta", Source::Human, 7),
            edge("k", "alpha", Source::Human, 7),
        ]);
        assert_eq!(a, b, "an exact tie must not depend on arrival order");
    }
}
