//! `split` — pull some members out of a case into a NEW case (case-plane scope).
//!
//! The inverse of [`crate::merge`], and the human's escape hatch from a reactor's grouping: the
//! storm folder decided these six detections were one incident, the technician on site knows two of
//! them are a separate fault. Without `split` the only way out is to resolve a case as
//! `false_positive` and lose its history.
//!
//! The new case is [`Grouping::Human`] — a person grouped it, and no reactor may re-fold it later.
//! Every moved member is stamped `human_placed`, which is that decision made durable.
//!
//! Remove-then-add per member, exactly as `merge` does and for the same reason: the exclusivity
//! invariant must hold at every instant, not merely at the end.

use lb_store::Store;

use crate::case::{Case, Grouping};
use crate::case_event::EventKind;
use crate::case_member::MemberRole;
use crate::error::CasesError;
use crate::event_append::append_event;
use crate::member_add::member_add;
use crate::member_remove::member_remove;
use crate::members::members_all;
use crate::open::{open, OpenInput};

/// Split `insight_ids` out of `from_id` into a new case titled `title`, as `actor`.
///
/// The first id becomes the new case's primary. Refuses an empty selection, an id that is not a
/// member of `from_id`, and a split that would empty the source case (that gesture is a re-title,
/// not a split — and it would leave a case citing nothing).
// SCOPE: docs/scope/insights/case-plane-scope.md §"Verbs" (case.split)
pub async fn split(
    store: &Store,
    ws: &str,
    from_id: &str,
    insight_ids: &[String],
    title: &str,
    actor: &str,
    ts: u64,
) -> Result<Case, CasesError> {
    let Some(from) = crate::get::get(store, ws, from_id).await? else {
        return Err(CasesError::BadInput(format!("no such case: {from_id}")));
    };
    if insight_ids.is_empty() {
        return Err(CasesError::BadInput(
            "case.split needs at least one insight to move out".into(),
        ));
    }
    let existing = members_all(store, ws, from_id).await?;
    for id in insight_ids {
        if !existing.iter().any(|m| &m.insight_id == id) {
            return Err(CasesError::BadInput(format!(
                "insight {id} is not a member of case {from_id} — nothing was moved"
            )));
        }
    }
    if insight_ids.len() >= existing.len() {
        return Err(CasesError::BadInput(format!(
            "splitting all {} members out of case {from_id} would leave it citing nothing — \
             re-title the case instead",
            existing.len()
        )));
    }

    // Remove the primary-to-be FIRST so `open` (which adds it through the invariant-enforcing
    // `member_add`) is not refused by the membership it is about to replace.
    let primary = insight_ids[0].clone();
    member_remove(store, ws, from_id, &primary).await?;

    let new_case = open(
        store,
        ws,
        OpenInput {
            title: title.to_string(),
            grouping: Grouping::Human,
            primary_insight: primary.clone(),
            severity: from.severity.clone(),
            category: from.category.clone(),
            site: from.site.clone(),
            scope: from.scope.clone(),
            assigned_to: from.assigned_to.clone(),
            caveated: from.caveated,
            // A person split this out. No reactor may fold it back.
            human_placed: true,
        },
        actor,
        ts,
    )
    .await?;

    for id in insight_ids.iter().skip(1) {
        member_remove(store, ws, from_id, id).await?;
        member_add(
            store,
            ws,
            &new_case.id,
            id,
            MemberRole::Explained,
            actor,
            true,
            ts,
        )
        .await?;
    }

    let payload = serde_json::json!({
        "from": from_id,
        "into": new_case.id,
        "moved": insight_ids,
    });
    append_event(
        store,
        ws,
        from_id,
        EventKind::Split,
        actor,
        payload.clone(),
        ts,
    )
    .await?;
    append_event(
        store,
        ws,
        &new_case.id,
        EventKind::Split,
        actor,
        payload,
        ts,
    )
    .await?;

    Ok(new_case)
}
