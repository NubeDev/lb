//! The `case_id` back-reference echo — one call, wrapped once (case-plane scope, resolved
//! decision 4).
//!
//! The membership fact lives in `case_member`; this is the echo of it on the insight so a roster
//! renders 200 case chips without 200 extra reads. Host-computed, never caller-supplied,
//! self-healing on the next grouping pass — the `producer` / tag-echo discipline.
//!
//! It is wrapped here rather than called inline in six places for one reason: an echo failure must
//! **never** fail the caller. The durable facts (the case, the membership) have already landed by
//! the time this runs, so an unwritable echo is a stale projection — precisely the state the next
//! reconcile pass repairs. A `?` here would turn a cosmetic staleness into a failed raise.

use lb_store::Store;

/// Write `case_id` onto `insight_id`'s back-ref. Best-effort and loud: logged, never propagated.
pub(super) async fn write_case_echo(store: &Store, ws: &str, insight_id: &str, case_id: &str) {
    if let Err(e) = lb_insights::set_case_id(store, ws, insight_id, Some(case_id)).await {
        tracing::warn!(
            ws, insight_id, case_id, error = %e,
            "case back-ref echo not written; the membership is durable and the next reconcile pass repairs it"
        );
    }
}

/// Re-echo the case id onto every member of `case_id` — what [`super::merge`] and [`super::split`]
/// owe after they move citations. Best-effort per member, for the reason above.
pub(super) async fn echo_members_of(
    node: &std::sync::Arc<crate::boot::Node>,
    ws: &str,
    case_id: &str,
) {
    // `members_all`, not the paged `members`: this walk wants the membership, and the paged read
    // resolves each insight's title for a drawer nobody is looking at here.
    let members = match lb_cases::members_all(&node.store, ws, case_id).await {
        Ok(members) => members,
        Err(e) => {
            tracing::warn!(ws, case_id, error = %e, "case echo skipped: members unreadable");
            return;
        }
    };
    for member in members {
        write_case_echo(&node.store, ws, &member.insight_id, case_id).await;
    }
}
