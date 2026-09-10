//! `set_case_id` — write the back-reference to the case that owns this finding
//! (`docs/scope/insights/case-plane-scope.md` §"Data model", resolved decision 4).
//!
//! An insight is a detection; a case is a piece of work. The membership fact lives in the case
//! plane (`case_member`), which is the source of truth. This is the **echo** of it on the insight,
//! for exactly one reason: a roster that lists 200 findings must render each one's case chip
//! without 200 extra reads. Same discipline as `producer` and the tag echo — host-computed, never
//! caller-supplied, self-healing on the next grouping pass.
//!
//! **This crate does not group.** Nothing here decides which case a finding belongs to; that is the
//! grouping reactor's job in the case plane. This file only persists the answer it is handed, which
//! is what keeps `lb-insights` ignorant of what a case is.
//!
//! One responsibility: the case back-ref write + its skip-when-unchanged guard.

use lb_store::{write, Store};

use crate::error::InsightsError;
use crate::get::get;
use crate::insight::{Insight, OCC_TABLE};
use crate::insight_id::record_id;

/// Set (or clear, with `None`) the case back-ref on the insight at `(ws, insight_id)`.
///
/// **The write is skipped when the value already matches** — the same discipline
/// [`crate::set_tags_echo`] holds, and for the same reason: the grouping reconcile loop re-derives
/// every open finding's case on every pass, so an unconditional write would rewrite the whole
/// insight table on a timer and bury every real change in revision churn.
///
/// A missing insight is `Ok(())`, not an error: the record may have been deleted between the
/// grouping decision and this write, and a reactor must not fail a pass over a row that is gone.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Resolved decisions" (4)
pub async fn set_case_id(
    store: &Store,
    ws: &str,
    insight_id: &str,
    case_id: Option<&str>,
) -> Result<(), InsightsError> {
    let Some(mut insight) = get(store, ws, insight_id).await? else {
        return Ok(());
    };
    let next = case_id.map(str::to_string);
    if insight.case_id == next {
        return Ok(());
    }
    insight.case_id = next;
    write_back(store, ws, &insight).await
}

async fn write_back(store: &Store, ws: &str, insight: &Insight) -> Result<(), InsightsError> {
    let value = serde_json::to_value(insight)
        .map_err(|e| InsightsError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    write(store, ws, OCC_TABLE, &record_id(&insight.id), &value).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::insight::OCC_TABLE as TABLE;
    use crate::origin::{Origin, OriginKind};
    use crate::pattern::{month_of_ts, Pattern};
    use crate::raise::{raise, RaiseInput};
    use crate::severity::Severity;
    use lb_store::read;

    /// Real embedded store, real `raise` verb (CLAUDE §9 — no mocks).
    async fn seed(store: &Store, ws: &str, key: &str, ts: u64) -> String {
        raise(
            store,
            ws,
            RaiseInput {
                dedup_key: key.into(),
                severity: Severity::Warning,
                title: "t".into(),
                body: serde_json::Value::Null,
                evidence: None,
                analysis: None,
                origin: Origin::new(OriginKind::Rule, "rule:x", None),
                tags: Default::default(),
                occurrence: None,
                ts,
                producer: "user:test".into(),
            },
            8,
        )
        .await
        .expect("raise")
        .id
    }

    /// The back-ref round-trips, and — the discipline that matters — an unchanged value writes
    /// NOTHING. The reconcile loop re-derives every open finding's case on every pass, so an
    /// unconditional write would rewrite the whole table on a timer.
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn the_back_ref_round_trips_and_an_unchanged_value_writes_nothing() {
        let store = Store::memory().await.unwrap();
        let ws = "nube";
        let id = seed(&store, ws, "k1", 1_721_001_600_000).await;

        set_case_id(&store, ws, &id, Some("case:abc")).await.unwrap();
        let got = crate::get::get(&store, ws, &id).await.unwrap().unwrap();
        assert_eq!(got.case_id.as_deref(), Some("case:abc"));

        // `rev` is the store's own revision counter; a skipped write leaves it untouched.
        let rev_of = |v: &serde_json::Value| v.get("rev").cloned();
        let before = read(&store, ws, TABLE, &id).await.unwrap();
        set_case_id(&store, ws, &id, Some("case:abc")).await.unwrap();
        let after = read(&store, ws, TABLE, &id).await.unwrap();
        assert_eq!(
            before.as_ref().and_then(rev_of),
            after.as_ref().and_then(rev_of),
            "re-setting the same case id must not touch the record"
        );

        // Clearing is a real change.
        set_case_id(&store, ws, &id, None).await.unwrap();
        let got = crate::get::get(&store, ws, &id).await.unwrap().unwrap();
        assert_eq!(got.case_id, None);

        // A finding that vanished between the grouping decision and the write is not an error.
        set_case_id(&store, ws, "no-such-insight", Some("case:abc"))
            .await
            .expect("a missing record is Ok(()), never an error a reactor pass dies on");
    }

    /// The month counters and the pattern are DERIVED on every raise — asserted through the real
    /// verb rather than by calling the pure fn, because the wiring is the part that can be missing.
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn every_raise_bumps_the_calendar_month_and_re_derives_the_pattern() {
        let store = Store::memory().await.unwrap();
        let ws = "nube";
        // 2024-07-15 — July.
        let july = 1_721_001_600_000u64;
        let id = seed(&store, ws, "k2", july).await;
        let got = crate::get::get(&store, ws, &id).await.unwrap().unwrap();
        assert_eq!(got.month_hist[month_of_ts(july)], 1);
        assert_eq!(got.pattern, Pattern::New, "one firing has no shape yet");

        // A second firing in a different calendar month lands in a different bucket.
        let december = 1_734_220_800_000u64; // 2024-12-15
        seed(&store, ws, "k2", december).await;
        let got = crate::get::get(&store, ws, &id).await.unwrap().unwrap();
        assert_eq!(got.month_hist[month_of_ts(july)], 1);
        assert_eq!(got.month_hist[month_of_ts(december)], 1);
        assert_eq!(got.count, 2, "the same dedup key — one record, two firings");
    }

    /// A record written BEFORE any of the four new fields existed must still decode, and must not
    /// gain keys it never had. The `filter_map(ok())` read path turns a schema mistake into
    /// silently-missing rows, so this is the guard that matters most.
    #[test]
    fn a_pre_existing_record_decodes_without_the_new_fields() {
        let old = serde_json::json!({
            "id": "i1", "dedup_key": "k", "severity": "warning", "title": "t",
            "origin": { "kind": "rule", "ref": "r" }, "status": "open",
            "count": 1, "first_ts": 1, "last_ts": 1, "producer": "user:test"
        });
        let insight: Insight = serde_json::from_value(old).expect("pre-existing shape decodes");
        assert!(insight.caveats.is_empty());
        assert_eq!(insight.case_id, None);
        assert_eq!(insight.month_hist, [0u32; 12]);
        assert_eq!(insight.pattern, Pattern::New);

        let back = serde_json::to_value(&insight).unwrap();
        for key in ["caveats", "case_id", "month_hist"] {
            assert!(back.get(key).is_none(), "empty ⇒ no key ({key}): {back}");
        }
    }
}
