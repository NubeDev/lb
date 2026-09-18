//! **Refresh a case's facet echo** — the repair half of the workspace facets.
//!
//! `Case.category`/`site`/`scope`/`subsystem` are ECHOES of the primary insight's tags, written
//! when the case opens (`crate::open`). Open-time-only is the right rule for a facet that was
//! already there: re-deriving it on every raise would let a producer silently move a case between
//! queues under an operator who is mid-triage.
//!
//! It is the WRONG rule for a facet that did not exist yet. When a new facet key ships — a pack
//! starts tagging `subsystem`, a dimension is added to the grammar — every case opened before that
//! carries `None` for ever, and no amount of re-raising repairs it because the echo only ever runs
//! at open. The queue then shows a filter axis that is real, enabled, and blank for the entire
//! existing estate, which reads to an operator as a broken feature rather than as missing history.
//!
//! So this fills **gaps only**: `None` may become `Some`, and nothing else. A facet the case
//! already carries is never rewritten, which is what keeps this safe to run on a timer and safe to
//! run twice — a producer that re-tags an insight cannot use this path to move a case out from
//! under the person working it. That asymmetry is the whole design, not an optimisation.
//!
//! Idempotent by construction: nothing to fill ⇒ no write, no event.

use lb_store::Store;

use crate::case::Case;
use crate::case_event::EventKind;
use crate::error::CasesError;
use crate::event_append::append_event;
use crate::save::save;

/// The facet gaps to fill on one case. Every field is the value read from the primary insight;
/// `None` means the insight does not carry that facet either, so there is nothing to fill.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FacetFill {
    pub category: Option<String>,
    pub site: Option<String>,
    pub scope: Option<String>,
    pub subsystem: Option<String>,
}

/// Fill any facet `id` is MISSING from `fill`, leaving every facet it already carries alone.
///
/// Returns the updated case, or `None` when nothing was missing — so a caller can count real
/// repairs without re-reading. A closed case is left alone: the record a client was shown at
/// resolution does not get rewritten afterwards, which is the same rule
/// [`crate::refresh_caveat`] holds.
///
/// `ts` is used for the history event only. The case's `last_activity_ts` is deliberately NOT
/// bumped (see below) — a migration is not activity, and an estate-wide backfill that touched it
/// would rewrite the queue's "last activity" column to the moment of the upgrade for every case at
/// once, destroying the one signal that column carries.
pub async fn refresh_facets(
    store: &Store,
    ws: &str,
    id: &str,
    fill: &FacetFill,
    actor: &str,
    ts: u64,
) -> Result<Option<Case>, CasesError> {
    let Some(mut case) = crate::get::get(store, ws, id).await? else {
        return Err(CasesError::BadInput(format!("no such case: {id}")));
    };
    if case.closed {
        return Ok(None);
    }

    let mut filled = serde_json::Map::new();
    fill_gap(&mut case.category, &fill.category, "category", &mut filled);
    fill_gap(&mut case.site, &fill.site, "site", &mut filled);
    fill_gap(&mut case.scope, &fill.scope, "scope", &mut filled);
    fill_gap(
        &mut case.subsystem,
        &fill.subsystem,
        "subsystem",
        &mut filled,
    );
    if filled.is_empty() {
        return Ok(None);
    }

    // Preserved across the write: `save` stamps `last_activity_ts = ts` for the eight verbs that
    // ARE activity, and this is the one caller that is not. Restoring it after the field edit is
    // cheaper than a second save path, and it keeps that stamping in exactly one file.
    let untouched = case.last_activity_ts;
    save(store, ws, &mut case, ts).await?;
    if case.last_activity_ts != untouched {
        case.last_activity_ts = untouched;
        save(store, ws, &mut case, untouched).await?;
    }

    // Stated in the history: a facet that appeared with no event is a case that silently changed
    // which queue it answers to, and "why is this in the water list now?" must be answerable.
    append_event(
        store,
        ws,
        id,
        EventKind::Facet,
        actor,
        serde_json::Value::Object(filled),
        ts,
    )
    .await?;
    Ok(Some(case))
}

/// Write `from` into `slot` when `slot` is empty and `from` is not, recording what was filled.
/// A facet the case already carries is never overwritten — the asymmetry the module doc explains.
fn fill_gap(
    slot: &mut Option<String>,
    from: &Option<String>,
    key: &str,
    filled: &mut serde_json::Map<String, serde_json::Value>,
) {
    if slot.is_some() {
        return;
    }
    let Some(value) = from else { return };
    *slot = Some(value.clone());
    filled.insert(key.to_string(), serde_json::Value::String(value.clone()));
}
