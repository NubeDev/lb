//! `backfill_insight_facets` — a one-shot boot job that re-folds every insight's tag echo through
//! the precedence rule (`docs/scope/insights/insight-tag-precedence-scope.md`).
//!
//! The echo is refreshed on every raise, so a corrected finding self-heals **on its next firing**.
//! The blind spot is the finding that never fires again: a resolved fault, a decommissioned meter,
//! a rule that was deleted. Its echo keeps whatever last-write-wins left there — which is precisely
//! the operator correction the fold exists to preserve, still reverted, for ever. The scope names
//! this and says the fold and the backfill "should probably land together". They do.
//!
//! Modelled on [`heal_insight_timestamps`](super::heal_insight_timestamps): a one-shot pass at boot,
//! best-effort per workspace, **idempotent by construction** — it writes only where the folded map
//! differs from the stored one, so a second run finds nothing to move and returns 0. Unlike the ts
//! heal it cannot be a single `UPDATE`: the answer for each row comes from that row's own tag edges,
//! so it is a read-fold-compare per insight. That is why it is a boot job and not a hot path.
//!
//! One responsibility: the echo backfill pass.

use std::collections::BTreeMap;

use lb_insights::{list, ListQuery};
use lb_store::Store;

use super::error::InsightSvcError;
use super::facets::fold_facets;

/// How many insights to pull per page. The list verb clamps at 500; paging keeps the pass's peak
/// memory bounded by the page rather than by the workspace's whole insight table.
const PAGE: usize = 500;

/// Re-materialize every insight's tag echo in `ws` from the tag graph, folded by precedence, and
/// persist the ones that moved. Returns **how many rows changed** — the number a boot log should
/// state, and the number a second run must report as 0.
///
/// Reads the graph raw (`lb_tags::of`) and writes through `lb_insights::set_tags_echo`, exactly as
/// the raise path does: one code path decides what an echo is, so the backfill can never disagree
/// with the live writer about the answer. `set_tags_echo` additionally skips the write when the map
/// already matches, so the idempotence is enforced twice — here, by the comparison that decides
/// whether to call at all, and there, by the guard that makes the call free if we were wrong.
///
/// Best-effort per row: an insight whose graph read or echo write fails is logged and skipped, never
/// aborting the pass. A backfill that stops at the first bad row leaves the rest silently unhealed,
/// which is worse than a partial pass that says so.
// SCOPE: docs/scope/insights/insight-tag-precedence-scope.md §"What building it touches"
pub async fn backfill_insight_facets(store: &Store, ws: &str) -> Result<usize, InsightSvcError> {
    let mut moved = 0usize;
    let mut cursor = None;
    loop {
        let page = list(
            store,
            ws,
            ListQuery {
                filter: Default::default(),
                cursor: cursor.clone(),
                limit: PAGE,
            },
            None,
            None,
        )
        .await?;

        for insight in &page.items {
            let entity = format!("insight:{}", insight.id);
            let applied = match lb_tags::of(store, ws, &entity).await {
                Ok(applied) => applied,
                Err(e) => {
                    tracing::warn!(%ws, id = %insight.id, error = ?e, "echo backfill: tag read skipped");
                    continue;
                }
            };
            // An entity with NO edges is left alone rather than blanked: the raise path's fallback
            // is "this raise's declared tags", so an empty graph read there means "graph hiccup",
            // and blanking a populated echo from a backfill would destroy data the fold is supposed
            // to protect.
            if applied.is_empty() {
                continue;
            }
            let folded: BTreeMap<String, String> = fold_facets(applied);
            if folded == insight.tags {
                continue;
            }
            match lb_insights::set_tags_echo(store, ws, &insight.id, &folded).await {
                Ok(_) => moved += 1,
                Err(e) => {
                    tracing::warn!(%ws, id = %insight.id, error = %e, "echo backfill: echo not written");
                }
            }
        }

        match page.next {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    if moved > 0 {
        tracing::info!(%ws, moved, "backfilled insight tag echoes (human > producer fold)");
    }
    Ok(moved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_insights::{raise, Origin, OriginKind, RaiseInput, Severity};
    use lb_tags::{Provenance, Source, Tag, DEFAULT_TAG_NODE_CAP};

    /// Real embedded store + real tag graph (CLAUDE §9 — no mocks). The record is created through
    /// the real `raise` verb and every edge through the real `lb_tags::add`.
    async fn seed(store: &Store, ws: &str, dedup_key: &str) -> String {
        let out = raise(
            store,
            ws,
            RaiseInput {
                dedup_key: dedup_key.into(),
                severity: Severity::Warning,
                title: "t".into(),
                body: serde_json::Value::Null,
                evidence: None,
                analysis: None,
                origin: Origin {
                    kind: OriginKind::Rule,
                    reference: "rule:x".into(),
                    run: None,
                },
                tags: Default::default(),
                occurrence: None,
                ts: 1,
                producer: "user:test".into(),
            },
            8,
        )
        .await
        .expect("raise");
        out.id
    }

    async fn tag(store: &Store, ws: &str, entity: &str, k: &str, v: &str, src: Source, at: u64) {
        let prov = Provenance::new(at, "user:test", src);
        lb_tags::add(
            store,
            ws,
            entity,
            &Tag::new(k.to_string(), serde_json::json!(v)),
            &prov,
            DEFAULT_TAG_NODE_CAP,
        )
        .await
        .expect("tag added");
    }

    /// **THE POINT OF THE JOB.** A finding that will never fire again carries a stale echo: the
    /// producer's value, left there by the pre-fold last-write-wins path, over the human's
    /// correction. Nothing heals it without this pass. Then: a second pass moves NOTHING.
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn it_folds_a_stale_echo_and_is_idempotent() {
        let store = Store::memory().await.unwrap();
        let ws = "nube";
        let id = seed(&store, ws, "k1").await;
        let entity = format!("insight:{id}");

        // Two coexisting edges for one key — the exact shape the graph's `(entity, tag, source)`
        // identity allows, and the one a flat echo has to pick from.
        tag(&store, ws, &entity, "classification", "plumbing", Source::Producer, 20).await;
        tag(&store, ws, &entity, "classification", "mechanical", Source::Human, 10).await;

        // Force the record into the stale state a pre-fold node would have left it in.
        let mut stale = BTreeMap::new();
        stale.insert("classification".to_string(), "plumbing".to_string());
        lb_insights::set_tags_echo(&store, ws, &id, &stale)
            .await
            .expect("stale echo seeded");

        let moved = backfill_insight_facets(&store, ws).await.expect("pass ok");
        assert_eq!(moved, 1, "the one stale row moved");

        let healed = lb_insights::get(&store, ws, &id).await.unwrap().unwrap();
        assert_eq!(
            healed.tags.get("classification").map(String::as_str),
            Some("mechanical"),
            "the human's correction is restored without the finding ever firing again"
        );

        // Idempotence — the property that lets this run on every boot.
        let again = backfill_insight_facets(&store, ws).await.expect("pass ok");
        assert_eq!(again, 0, "a second run moves nothing");
    }

    /// An insight with NO tag edges must be left alone, not blanked. Guards the failure mode where
    /// a boot job quietly destroys the data it was added to protect.
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn a_finding_with_no_edges_is_not_blanked() {
        let store = Store::memory().await.unwrap();
        let ws = "nube";
        let id = seed(&store, ws, "k2").await;

        let mut echo = BTreeMap::new();
        echo.insert("building".to_string(), "north".to_string());
        lb_insights::set_tags_echo(&store, ws, &id, &echo).await.unwrap();

        assert_eq!(backfill_insight_facets(&store, ws).await.unwrap(), 0);
        let after = lb_insights::get(&store, ws, &id).await.unwrap().unwrap();
        assert_eq!(after.tags, echo, "an empty graph read never blanks a populated echo");
    }
}
