//! The **case-group reactor** — find or open the case for a just-raised insight (case-plane scope).
//!
//! The invariant it serves: **every open insight is in exactly one open case.** That is why this
//! runs INLINE at the end of `insight_raise` and not only on a loop — a scan-only reactor makes the
//! invariant *eventually* true, so a UI that raises then lists sees a case-less row for one tick.
//! The reconcile loop (`super::reconcile`) is the restart-safe backstop and calls this same
//! function, so there is one grouping implementation and not two that drift.
//!
//! **The decision, in order:**
//!   1. Already in an open case ⇒ that case (and a severity escalation **punctures** its snooze).
//!   2. A **verdict** record (`body.explains[]` non-empty) ⇒ the case is about the ROOT it names,
//!      with every finding it explains — and the verdict record itself — as `explained` members.
//!   3. An open verdict case already **cites** this finding ⇒ join it (the verdict-first ordering).
//!   4. The last case for this finding closed as `fixed` inside the hold-down window ⇒ reopen it
//!      (`super::hold_down`).
//!   5. Otherwise ⇒ one insight, one case, `grouping: single`.
//!
//! **Rule 10.** Nothing here names a pack, a rule, a category value or an extension. The only thing
//! read out of `body` is the generic citation grammar, and it is read in ONE file
//! ([`super::verdict`]) so this one cannot quietly learn a second key.
//!
//! **`human_placed` is absolute.** Every merge this file performs passes `skip_human_placed: true`.
//! A person who moved a detection into a case made a judgement the machine cannot see, and no
//! reactor may undo it — not to fix a grouping, not to satisfy an invariant.

use std::sync::Arc;

use lb_cases::{Case, Grouping, MemberRole, OpenInput};
use lb_insights::Insight;
use lb_store::Store;

use super::error::CaseSvcError;
use super::facets::facets_of;
use crate::boot::Node;

/// The subject the grouping reactor attributes its writes to. `system:` — not a user, and visibly
/// not one in the case history.
pub const GROUP_ACTOR: &str = "system:case-group";

/// Find or open the case for `insight_id`, run the SLA clock over it, and return its id.
///
/// The clock runs HERE, on the tail, rather than at each of [`decide`]'s five exits — and it runs
/// on every pass, not only when a case is opened. Both halves are deliberate:
///   * one call site means the open path and the **severity-escalation** path (exit 1, which
///     punctures a snooze and writes the higher severity) cannot drift apart. Severity is a
///     policy-match axis, so a case that got worse must be re-measured against whatever clause
///     covers the worse thing;
///   * running it unconditionally is free, because the deadlines are a pure function of
///     `(policy, opened_ts)` and `lb_cases::set_deadlines` writes nothing when the answer is
///     unchanged. That is also what makes "a snooze never moves the clock" structural rather than a
///     rule somebody has to remember (see [`super::sla_clock`]).
// SCOPE: docs/scope/insights/case-plane-scope.md §"Reactors" (case-group, sla-clock)
pub async fn group_insight(
    node: &Arc<Node>,
    ws: &str,
    insight_id: &str,
    now: u64,
) -> Result<String, CaseSvcError> {
    let case_id = decide(node, ws, insight_id, now).await?;
    super::sla_clock::apply_sla(node, ws, &case_id, now).await?;
    Ok(case_id)
}

/// The grouping decision itself — the five-way ladder in the module doc. Split out so the SLA clock
/// above has exactly one place to hang rather than five returns to remember.
async fn decide(
    node: &Arc<Node>,
    ws: &str,
    insight_id: &str,
    now: u64,
) -> Result<String, CaseSvcError> {
    let store = &node.store;
    let Some(insight) = lb_insights::get(store, ws, insight_id).await? else {
        return Err(CaseSvcError::BadInput(format!(
            "no such insight: {insight_id}"
        )));
    };
    let severity = super::facets::severity_str(&insight);

    // 1. Already grouped. The one write this arm may make is the snooze puncture: a case somebody
    //    parked that has since become critical is not still parked.
    if let Some((case, _)) = lb_cases::find_open_case_for_insight(store, ws, insight_id).await? {
        lb_cases::puncture_snooze(store, ws, &case.id, &severity, GROUP_ACTOR, now).await?;
        // The caveat echo is LIVE, not a snapshot taken when the case opened. `lb_insights::raise`
        // recomputes `Insight.caveats` on every firing on purpose — a caveat describes the world
        // right now — so a case whose echo is frozen at open drifts away from its own evidence in
        // whichever direction hurts: it hides the soft-block on a case whose sensor broke after it
        // opened (a contractor dispatched against a number nobody trusts, the exact outcome the
        // caveat exists to prevent), and it keeps the badge on a case whose sensor has since been
        // fixed (the permanent caveat the insight layer refuses to create).
        //
        // Only the PRIMARY decides: `Case.caveated` echoes the primary insight's state, so a
        // caveated member must not switch the whole case's contractor button off.
        if case.primary_insight == insight_id {
            let caveated = super::facets::facets_of(&insight).caveated;
            lb_cases::refresh_caveat(store, ws, &case.id, caveated, GROUP_ACTOR, now).await?;
        }
        super::echo::write_case_echo(store, ws, insight_id, &case.id).await;
        return Ok(case.id);
    }

    // 2. A verdict record — it names a root cause and the findings that root explains.
    if let Some(verdict) = super::verdict::resolve_verdict(store, ws, &insight).await? {
        let case_id = group_verdict(node, ws, &insight, &verdict, now).await?;
        return Ok(case_id);
    }

    // 3. An open verdict case already cites this finding (it arrived before this straggler). The
    //    most specific claim on a finding is a live case that explicitly named it, so this outranks
    //    the hold-down check below.
    if let Some(case) = super::cites::find_citing_case(store, ws, insight_id).await? {
        lb_cases::member_add(
            store,
            ws,
            &case.id,
            insight_id,
            MemberRole::Explained,
            GROUP_ACTOR,
            false,
            now,
        )
        .await?;
        super::echo::write_case_echo(store, ws, insight_id, &case.id).await;
        return Ok(case.id);
    }

    // 4. The repair may not have held — reopen the case that fixed it rather than opening a new one.
    if let Some(case_id) = super::hold_down::reopen_if_held(node, ws, &insight, now).await? {
        super::echo::write_case_echo(store, ws, insight_id, &case_id).await;
        return Ok(case_id);
    }

    // 5. One insight, one case.
    let case = open_for(store, ws, &insight, Grouping::Single, insight_id, now).await?;
    super::echo::write_case_echo(store, ws, insight_id, &case.id).await;
    Ok(case.id)
}

/// The verdict arm: the case is about the ROOT, and every finding the root explains — plus the
/// verdict record that named it — is an `explained` member.
async fn group_verdict(
    node: &Arc<Node>,
    ws: &str,
    citing: &Insight,
    verdict: &super::verdict::Verdict,
    now: u64,
) -> Result<String, CaseSvcError> {
    let store = &node.store;
    let target = verdict_case(node, ws, citing, &verdict.primary, now).await?;

    // Every cited finding, then the citing record itself. `fold_into` handles all three states a
    // finding can be in: free, already in this case, or sitting in another open case that must be
    // merged in (the verdict-LAST ordering).
    for id in &verdict.explained {
        fold_into(node, ws, &target, id, now).await?;
    }
    if citing.id != target {
        fold_into(node, ws, &target, &citing.id, now).await?;
    }

    super::echo::echo_members_of(node, ws, &target).await;
    let _ = store;
    Ok(target)
}

/// The case the verdict's ROOT belongs in — reusing an existing one where that is honest, opening a
/// new `verdict` case where it is not.
async fn verdict_case(
    node: &Arc<Node>,
    ws: &str,
    citing: &Insight,
    primary: &str,
    now: u64,
) -> Result<String, CaseSvcError> {
    let store = &node.store;
    let Some(insight) = lb_insights::get(store, ws, primary).await? else {
        return Err(CaseSvcError::BadInput(format!(
            "no such insight: {primary}"
        )));
    };

    let Some((existing, member)) = lb_cases::find_open_case_for_insight(store, ws, primary).await?
    else {
        // The root is free — open the verdict case over it directly.
        let case = open_for(store, ws, &insight, Grouping::Verdict, primary, now).await?;
        return Ok(case.id);
    };

    // Already a verdict case for this root: reuse it. Re-raising the verdict record every 15
    // minutes must not mint a case each time.
    if existing.grouping == Grouping::Verdict {
        return Ok(existing.id);
    }

    // A person put the root where it is. We may add to that case, but we may not move the root out
    // of it — so THAT case becomes the verdict's home, keeping its human grouping.
    if member.human_placed {
        tracing::info!(
            ws, case_id = %existing.id, %primary, citing = %citing.id,
            "verdict root is human-placed; folding the citation into the case a person chose rather than moving it"
        );
        return Ok(existing.id);
    }

    // The root sits in a machine-made case (a `single` opened before the verdict record arrived).
    // Free the root, open the verdict case over it, then merge the leftovers in — which closes the
    // old case as `duplicate` and moves any other members across, skipping anything human-placed.
    lb_cases::member_remove(store, ws, &existing.id, primary).await?;
    let case = open_for(store, ws, &insight, Grouping::Verdict, primary, now).await?;
    lb_cases::merge(store, ws, &existing.id, &case.id, GROUP_ACTOR, true, now).await?;
    Ok(case.id)
}

/// Put `insight_id` into `target`, whatever state it is currently in.
async fn fold_into(
    node: &Arc<Node>,
    ws: &str,
    target: &str,
    insight_id: &str,
    now: u64,
) -> Result<(), CaseSvcError> {
    let store = &node.store;
    match lb_cases::find_open_case_for_insight(store, ws, insight_id).await? {
        Some((case, _)) if case.id == target => Ok(()),
        Some((case, member)) if member.human_placed => {
            // The stop sign. A person put this detection in that case; the grouping is left wrong
            // rather than overruling them, and it is logged so the wrongness is visible.
            tracing::info!(
                ws, %insight_id, held_by = %case.id, %target,
                "not folding a human-placed member into the verdict case"
            );
            Ok(())
        }
        Some((case, _)) => {
            // The verdict-LAST ordering: this finding already had a case (typically a `single`
            // opened when it was raised). Merge it in rather than leaving a duplicate open case —
            // `merge` moves every member and closes the loser as `duplicate`.
            lb_cases::merge(store, ws, &case.id, target, GROUP_ACTOR, true, now).await?;
            Ok(())
        }
        None => {
            lb_cases::member_add(
                store,
                ws,
                target,
                insight_id,
                MemberRole::Explained,
                GROUP_ACTOR,
                false,
                now,
            )
            .await?;
            Ok(())
        }
    }
}

/// Open a case over `primary`, echoing the facets off the insight the case is about.
pub(super) async fn open_for(
    store: &Store,
    ws: &str,
    insight: &Insight,
    grouping: Grouping,
    primary: &str,
    now: u64,
) -> Result<Case, CaseSvcError> {
    let facets = facets_of(insight);
    Ok(lb_cases::open(
        store,
        ws,
        OpenInput {
            title: insight.title.clone(),
            grouping,
            primary_insight: primary.to_string(),
            severity: facets.severity,
            category: facets.category,
            site: facets.site,
            scope: facets.scope,
            // The triage backfill: a finding a person already owns opens a case they already own.
            assigned_to: insight.assigned_to.clone(),
            caveated: facets.caveated,
            // A reactor opened this. Nothing here is human-placed.
            human_placed: false,
        },
        GROUP_ACTOR,
        now,
    )
    .await?)
}
