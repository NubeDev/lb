//! `merge` — fold one case into another (case-plane scope).
//!
//! **This is the verb that MOVES members**, and it exists because [`crate::member_add`] refuses to
//! put an insight into a second open case. Remove-then-add, one member at a time, so the
//! exclusivity invariant is never violated even for an instant — a crash mid-merge leaves some
//! members moved and the rest where they were, which the next reconcile pass reads as consistent.
//!
//! The losing case closes as [`Resolution::Duplicate`], which is not bookkeeping: the hold-down
//! reactor treats `fixed` as "a repair that may not hold" and everything else as "let a new case
//! open", and a merged-away case must never be mistaken for a repair.
//!
//! **`human_placed` members are never moved by a reactor** — but they ARE moved here, because a
//! human asked for this merge. The stop sign lives in the reactors (`host/src/case/group.rs`),
//! which check the flag before calling anything in this file. That asymmetry is the design: a
//! person may overrule a person, a machine may not.

use lb_store::Store;

use crate::case::{Case, Resolution, Workflow};
use crate::case_event::EventKind;
use crate::case_member::MemberRole;
use crate::error::CasesError;
use crate::event_append::append_event;
use crate::member_add::member_add;
use crate::member_remove::member_remove;
use crate::members::members_all;

/// Merge `from_id` into `into_id` in workspace `ws` as `actor`.
///
/// Every member of `from_id` moves to `into_id` (keeping its `human_placed` flag and, for anything
/// that was the losing case's PRIMARY, taking the [`MemberRole::Duplicate`] role — there is exactly
/// one primary per case and the winner already has it). `from_id` then closes as `duplicate`.
///
/// `skip_human_placed` is how the REACTORS call this: `true` leaves a member a person put in the
/// losing case exactly where they put it. A human merge passes `false`.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Verbs" (case.merge)
pub async fn merge(
    store: &Store,
    ws: &str,
    from_id: &str,
    into_id: &str,
    actor: &str,
    skip_human_placed: bool,
    ts: u64,
) -> Result<Case, CasesError> {
    if from_id == into_id {
        return Err(CasesError::BadInput(
            "cannot merge a case into itself".into(),
        ));
    }
    let Some(from) = crate::get::get(store, ws, from_id).await? else {
        return Err(CasesError::BadInput(format!("no such case: {from_id}")));
    };
    if crate::get::get(store, ws, into_id).await?.is_none() {
        return Err(CasesError::BadInput(format!("no such case: {into_id}")));
    }

    let mut moved = Vec::new();
    let mut kept = Vec::new();
    for member in members_all(store, ws, from_id).await? {
        if skip_human_placed && member.human_placed {
            kept.push(member.insight_id);
            continue;
        }
        // Remove FIRST: `member_add` refuses an insight held by another open case, so the order is
        // the invariant, not a preference.
        member_remove(store, ws, from_id, &member.insight_id).await?;
        let role = if member.role == MemberRole::Primary {
            MemberRole::Duplicate
        } else {
            member.role
        };
        member_add(
            store,
            ws,
            into_id,
            &member.insight_id,
            role,
            actor,
            member.human_placed,
            ts,
        )
        .await?;
        moved.push(member.insight_id);
    }

    let payload = serde_json::json!({
        "from": from_id,
        "into": into_id,
        "moved": moved,
        "kept_human_placed": kept,
    });
    append_event(
        store,
        ws,
        into_id,
        EventKind::Merged,
        actor,
        payload.clone(),
        ts,
    )
    .await?;
    append_event(store, ws, from_id, EventKind::Merged, actor, payload, ts).await?;

    // Close the loser. Idempotent on an already-closed case: re-resolving as `duplicate` restates
    // the same fact, and refusing here would strand a half-done merge.
    let closed = if from.closed && from.resolution == Some(Resolution::Duplicate) {
        from
    } else {
        crate::workflow::workflow(
            store,
            ws,
            from_id,
            Workflow::Resolved,
            Some(Resolution::Duplicate),
            None,
            actor,
            ts,
        )
        .await?
    };
    Ok(closed)
}
