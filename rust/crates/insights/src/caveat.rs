//! `caveats` — "the data this finding rests on is itself under question"
//! (`docs/scope/insights/case-plane-scope.md` §"Data model").
//!
//! A finding derived from a stuck sensor is not a finding, it is a symptom of the stuck sensor. But
//! the rule that raised it cannot know that: it queried a series, the series answered, the number
//! was out of band. The knowledge that the series is untrustworthy lives in a **different, open
//! finding** — one raised against the same points, in the workspace's declared data-quality
//! category. This file joins the two at raise time so the second finding carries the first's id.
//!
//! What that buys, in order of importance: **the caveated finding never breaks through**
//! (`ladder.rs`) — nobody is paged at 2am about a number produced by a sensor we already know is
//! broken; and the drawer can say *why* this finding is soft, with a link, instead of the operator
//! rediscovering it.
//!
//! **Rule 10.** The gating category is a PARAMETER of this function, never a literal in this crate.
//! It comes from the workspace's own `tag_vocab:category` row (`vocab.rs`), which packs seed. A
//! workspace that declared no gating value gets an empty result and the whole feature is inert —
//! that is the correct behaviour, not a degraded one.
//!
//! One responsibility: find the open gating findings that overlap this finding's subjects.

use std::collections::BTreeSet;

use lb_store::Store;

use crate::error::InsightsError;
use crate::insight::{Insight, OCC_TABLE};
use crate::status::Status;
use crate::table_scan::scan_all;
use crate::vocab::CATEGORY_KEY;

/// The ids of the OPEN findings in `dq_category` whose evidence subjects intersect `subjects`,
/// excluding `self_id`. Sorted and deduplicated, so the stamped list is stable across raises (an
/// unstable list would rewrite the record on every firing and make every diff noise).
///
/// "Open" means `Open` **or** `Acked` — an acknowledged data-quality problem is still an unfixed
/// data-quality problem, and the operator who acked it did not thereby vouch for the data. Only
/// `Resolved` clears the caveat, which is what makes the caveat self-healing: resolve the sensor
/// finding and the next raise of the dependent one comes back uncaveated.
///
/// Returns empty (and reads nothing) when `subjects` or `dq_category` is empty — the unseeded
/// workspace path, and the path for every producer that states no `evidence.subjects`.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Data model" (`caveats`) + §"Testing plan"
pub async fn caveats_for(
    store: &Store,
    ws: &str,
    subjects: &[String],
    self_id: &str,
    dq_category: &str,
) -> Result<Vec<String>, InsightsError> {
    if subjects.is_empty() || dq_category.is_empty() {
        return Ok(Vec::new());
    }
    let wanted: BTreeSet<&str> = subjects.iter().map(String::as_str).collect();

    // One ws-scoped scan of the insight table — the same read `insight.list` makes, and the
    // workspace wall is structural (each page is `use_ws(ws)`). A row that fails to decode is
    // skipped, exactly as the list path does: a caveat is an enrichment, and one undecodable
    // neighbour must not fail a raise.
    let rows = scan_all(store, ws, OCC_TABLE).await?;
    let mut hits: BTreeSet<String> = BTreeSet::new();
    for row in rows {
        let Ok(other) = serde_json::from_value::<Insight>(row) else {
            continue;
        };
        if other.id == self_id {
            continue;
        }
        if !matches!(other.status, Status::Open | Status::Acked) {
            continue;
        }
        // The category is read from the tag ECHO, which the host materializes from the graph right
        // after each raise. That is deliberate: the echo is the union across all raises AND across
        // a human correction, so a finding an operator RE-classified into the gating category
        // starts gating on its next read — which is the whole point of the `Human > Producer` fold.
        if other.tags.get(CATEGORY_KEY).map(String::as_str) != Some(dq_category) {
            continue;
        }
        let overlaps = other
            .evidence
            .as_ref()
            .map(|e| e.subjects.iter().any(|s| wanted.contains(s.as_str())))
            .unwrap_or(false);
        if overlaps {
            hits.insert(other.id);
        }
    }
    Ok(hits.into_iter().collect())
}
