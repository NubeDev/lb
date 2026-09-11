//! Reading a **verdict** out of an insight's body — the one place the grouping reactor touches
//! `body` at all (case-plane scope, "case-group" reactor).
//!
//! A *verdict record* is a finding that CITES other findings: its body carries `root_cause` (the
//! upstream fault it blames) and `explains[]` (the downstream symptoms that fault accounts for).
//! Both are generic body keys documented in the scope as "a record that cites other findings".
//!
//! **Rule 10 lives or dies here.** This file reads exactly two keys — `root_cause` and `explains` —
//! and nothing else from `body`. It names no rule, no pack, no equipment key and no issue value; a
//! swapped-out producer writing the same two keys groups identically, which is the test. If you are
//! about to add a third key or a value check here, you are special-casing a producer.
//!
//! **The citations are `dedup_key` strings, not insight ids** — that is what the real producer
//! writes, because a rule knows the stable identity of the finding it blames, not the ULID some
//! other raise assigned it. Each is resolved through `lb_insights::dedup_lookup`, falling back to a
//! get-by-id so the seam is not brittle if another producer cites ids. An entry that resolves to
//! nothing is **skipped with a warning, never an error**: the citing record can legitimately arrive
//! before, or after, the records it cites, and the reconcile loop folds the stragglers in later.

use lb_insights::Insight;
use lb_store::Store;

use super::error::CaseSvcError;

/// The two body keys a verdict is made of. lb's own generic citation grammar — see the module doc.
const ROOT_CAUSE: &str = "root_cause";
const EXPLAINS: &str = "explains";

/// A resolved verdict: which finding is the ROOT and which findings it explains.
#[derive(Debug, Clone)]
pub(super) struct Verdict {
    /// The insight id of the root cause — the case's `primary_insight`.
    ///
    /// Falls back to the CITING record itself when `root_cause` is absent or unresolvable: a
    /// verdict that names symptoms but no cause is still a real grouping, and the alternative
    /// (refusing to group) would leave every cited finding in its own case.
    pub primary: String,
    /// Insight ids the root explains, resolved and de-duplicated. Excludes the primary.
    pub explained: Vec<String>,
}

/// True when `insight` is a verdict record — its body carries a non-empty `explains[]` array.
///
/// This is the whole grouping predicate, and it is a property of the DATA: no rule name, no
/// producer check.
pub(super) fn is_verdict(insight: &Insight) -> bool {
    insight
        .body
        .get(EXPLAINS)
        .and_then(|v| v.as_array())
        .is_some_and(|a| !a.is_empty())
}

/// Resolve `insight`'s verdict into insight ids, or `None` when it is not a verdict record.
///
/// The citing record itself is NOT in `explained` — the caller adds it as an `explained` member
/// separately, because it is a member of the case but is not something the root "explains".
pub(super) async fn resolve_verdict(
    store: &Store,
    ws: &str,
    insight: &Insight,
) -> Result<Option<Verdict>, CaseSvcError> {
    if !is_verdict(insight) {
        return Ok(None);
    }

    // The ROOT is the primary, not the record that named it. The scope's worked example is
    // explicit: the case is about the flatline; the verdict record is the record that says so.
    let primary = match insight.body.get(ROOT_CAUSE).and_then(|v| v.as_str()) {
        Some(cited) => match resolve_citation(store, ws, cited).await? {
            Some(id) => id,
            None => {
                tracing::warn!(
                    ws, citing = %insight.id, %cited,
                    "verdict root_cause does not resolve yet; grouping under the citing record until it does"
                );
                insight.id.clone()
            }
        },
        None => insight.id.clone(),
    };

    let cited = insight
        .body
        .get(EXPLAINS)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut explained: Vec<String> = Vec::with_capacity(cited.len());
    for entry in cited {
        let Some(key) = entry.as_str() else {
            tracing::warn!(ws, citing = %insight.id, "verdict explains[] entry is not a string; skipped");
            continue;
        };
        match resolve_citation(store, ws, key).await? {
            Some(id) if id == primary => {} // the root cannot explain itself
            Some(id) if explained.contains(&id) => {}
            Some(id) => explained.push(id),
            None => tracing::warn!(
                ws, citing = %insight.id, cited = %key,
                "verdict explains[] entry does not resolve yet; skipped — the reconcile loop folds it in when it exists"
            ),
        }
    }

    Ok(Some(Verdict { primary, explained }))
}

/// Resolve one citation to an insight id: `dedup_key` first (what the real producer writes), then
/// a get-by-id fallback so the seam is not brittle if another producer cites ids.
pub(super) async fn resolve_citation(
    store: &Store,
    ws: &str,
    cited: &str,
) -> Result<Option<String>, CaseSvcError> {
    if let Some(insight) = lb_insights::dedup_lookup(store, ws, cited).await? {
        return Ok(Some(insight.id));
    }
    Ok(lb_insights::get(store, ws, cited).await?.map(|i| i.id))
}
