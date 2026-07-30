//! `set_tags_echo` — persist the insight's tag facets onto the record as a read-only projection
//! (`docs/scope/insights/insight-tag-echo-scope.md`).
//!
//! Tags are the insight's dimension plane, but they live in the tag graph, not on the record — so a
//! roster that lists "every open insight in Chullora" gets rows back and then cannot say which
//! building each row is in without an N+1 `tags.find`. This verb writes the *materialized* facet
//! map onto the record so `insight.list` alone renders the dimension columns.
//!
//! **This crate stays tag-graph-agnostic** (README §7 — the graph is the host's): the caller hands
//! in an already-materialized `{k: v}` map. Where that map came from is the host's business, and
//! the scope pins it: `tags.of` on the insight entity (the union across ALL raises of the dedup
//! key), never one raise's declared `tags`.
//!
//! One responsibility: the echo write + its size guard.

use std::collections::BTreeMap;

use lb_store::{write, Store};

use crate::error::InsightsError;
use crate::get::get;
use crate::insight::{Insight, OCC_TABLE};
use crate::insight_id::record_id;

/// Serialized-size cap for the whole echo map. Half the evidence cap: the echo is a handful of
/// short low-cardinality dimension strings by contract (the tag plane's cardinality rule — identity
/// belongs in `dedup_key`), and unlike evidence it rides EVERY row of EVERY roster page.
pub const MAX_TAG_ECHO_BYTES: usize = 2 * 1024;

/// Validate a materialized facet map against [`MAX_TAG_ECHO_BYTES`] WITHOUT writing.
///
/// Unlike [`crate::validate_evidence_size`], an over-cap echo cannot reject the raise: the echo is
/// host-computed *after* the durable record already landed, and the tags it projects are already in
/// the graph. So the contract here is **loud skip, never silent truncation** — the caller gets an
/// `Err` to log and the stored echo is left at its previous value (visibly stale) rather than
/// silently half-written (invisibly wrong).
pub fn validate_tags_echo_size(tags: &BTreeMap<String, String>) -> Result<(), InsightsError> {
    let bytes = serde_json::to_vec(tags)
        .map_err(|e| InsightsError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    if bytes.len() > MAX_TAG_ECHO_BYTES {
        return Err(InsightsError::BadInput(format!(
            "tag echo {} bytes exceeds the {MAX_TAG_ECHO_BYTES}-byte cap — the echo carries low-cardinality DIMENSIONS (building, asset type, priority); per-entity identity belongs in dedup_key",
            bytes.len()
        )));
    }
    Ok(())
}

/// Write `tags` as the echo on the insight at `(ws, id)` and return the post-write record.
///
/// `Ok(None)` ⇒ no such insight in this workspace (nothing to echo onto — not an error, the record
/// may have been deleted between the raise and here). The write is **skipped when the echo already
/// matches**, so the steady-state re-raise of a flapping producer costs one read and no write.
///
/// Idempotent and safe to re-run — the backfill job reuses it verbatim.
// SCOPE: docs/scope/insights/insight-tag-echo-scope.md §"Intent / approach"
pub async fn set_tags_echo(
    store: &Store,
    ws: &str,
    id: &str,
    tags: &BTreeMap<String, String>,
) -> Result<Option<Insight>, InsightsError> {
    validate_tags_echo_size(tags)?;
    let Some(mut insight) = get(store, ws, id).await? else {
        return Ok(None);
    };
    if &insight.tags == tags {
        return Ok(Some(insight));
    }
    insight.tags = tags.clone();
    let value = serde_json::to_value(&insight)
        .map_err(|e| InsightsError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    write(store, ws, OCC_TABLE, &record_id(&insight.id), &value).await?;
    Ok(Some(insight))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facets(n: usize) -> BTreeMap<String, String> {
        (0..n)
            .map(|i| (format!("dimension-number-{i:03}"), "a-facet-value".into()))
            .collect()
    }

    #[test]
    fn a_dimension_sized_facet_map_passes_the_guard() {
        // The shape the tag plane's cardinality rule actually produces: a handful of short strings.
        assert!(validate_tags_echo_size(&facets(6)).is_ok());
        assert!(validate_tags_echo_size(&BTreeMap::new()).is_ok());
    }

    #[test]
    fn an_absurd_facet_map_is_rejected_whole_and_says_why() {
        let err = validate_tags_echo_size(&facets(200)).expect_err("over the cap");
        let msg = err.to_string();
        // The message must name the CONTRACT, not just the number — this error lands in a log line
        // a producer has to act on (their tags carry identity, not dimensions).
        assert!(msg.contains("dedup_key"), "actionable message: {msg}");
        assert!(
            msg.contains(&MAX_TAG_ECHO_BYTES.to_string()),
            "states the cap: {msg}"
        );
    }
}
