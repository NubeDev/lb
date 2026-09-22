//! `case_open` — a human opens a case over one or more insights (case-plane scope).
//!
//! Gated on `mcp:case.open:call`, the AUTHOR (member) tier: grouping detections into a piece of
//! work is an authoring act, not a read.
//!
//! Two things this layer adds over the crate's [`lb_cases::open`]:
//!   1. `opened_by` is **forced** to the principal's `sub` — the `ack.rs` host-stamp precedent. A
//!      caller supplying an actor is ignored, not refused: the field is simply not read from input.
//!   2. Every member is stamped `human_placed`. A person grouped these, so no reactor may re-fold
//!      them later — the stop sign the case-group reactor checks before it moves anything.
//!
//! The facets (`severity`/`category`/`site`/`scope`) are echoed from the PRIMARY insight rather
//! than taken from the caller: they are already a property of the detection, and a second writer
//! for them is a second thing that can be wrong.

use std::sync::Arc;

use lb_auth::Principal;
use lb_cases::{Case, Grouping, MemberRole, OpenInput};
use lb_mcp::authorize_tool;

use super::error::CaseSvcError;
use super::facets::facets_of;
use crate::boot::Node;

/// Open a case over `primary_insight` (plus any `also` insights) in `ws` as `principal`.
///
/// `grouping` defaults to [`Grouping::Human`] — a person did this. Refuses when an insight does not
/// exist here, and when one is already held by another OPEN case (the exclusivity invariant; merge
/// the cases instead).
pub async fn case_open(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    title: &str,
    primary_insight: &str,
    also: &[String],
    ts: u64,
) -> Result<Case, CaseSvcError> {
    authorize_tool(principal, ws, "case.open").map_err(|_| CaseSvcError::Denied)?;

    let store = &node.store;
    // Entity-scoped data: every cited insight must be inside the caller's entities (an out-of-scope
    // one reads exactly like a missing one).
    crate::insight::insight_ensure_visible(store, principal, ws, primary_insight)
        .await
        .map_err(insight_err)?;
    for id in also {
        crate::insight::insight_ensure_visible(store, principal, ws, id)
            .await
            .map_err(insight_err)?;
    }
    // Establish the primary exists in THIS workspace before writing anything — a case citing a
    // detection that is not here is a case about nothing.
    let Some(insight) = lb_insights::get(store, ws, primary_insight).await? else {
        return Err(CaseSvcError::BadInput(format!(
            "no such insight: {primary_insight}"
        )));
    };
    let facets = facets_of(&insight);
    let impact = super::impact::impact_of(&insight, facets.caveated);

    let case = lb_cases::open(
        store,
        ws,
        OpenInput {
            title: title.to_string(),
            grouping: Grouping::Human,
            primary_insight: primary_insight.to_string(),
            severity: facets.severity,
            category: facets.category,
            site: facets.site,
            scope: facets.scope,
            subsystem: facets.subsystem,
            assigned_to: insight.assigned_to.clone(),
            caveated: facets.caveated,
            // A person opening a case gets the same money echo a reactor would. The rate is a
            // property of the finding, not of who filed the work — and a case whose figure depended
            // on which door opened it would be the second writer the echo exists to avoid.
            impact_rate: impact.rate,
            impact_tier: impact.tier,
            human_placed: true,
        },
        principal.sub(),
        ts,
    )
    .await?;

    for id in also {
        if id == primary_insight {
            continue;
        }
        if lb_insights::get(store, ws, id).await?.is_none() {
            return Err(CaseSvcError::BadInput(format!("no such insight: {id}")));
        }
        lb_cases::member_add(
            store,
            ws,
            &case.id,
            id,
            MemberRole::Explained,
            principal.sub(),
            true,
            ts,
        )
        .await?;
    }

    // Echo the case id onto every member so a roster renders the case chip without an N+1.
    super::echo::write_case_echo(store, ws, primary_insight, &case.id).await;
    for id in also {
        super::echo::write_case_echo(store, ws, id, &case.id).await;
    }

    // The SLA clock. A human-opened case gets its deadline from the same one code path a
    // reactor-opened one does (`super::sla_clock`), so the two can never disagree about which
    // clause governs the work. Re-read so the caller is handed the case WITH its deadlines rather
    // than the pre-clock record.
    super::sla_clock::apply_sla(node, ws, &case.id, ts).await?;
    Ok(lb_cases::get(store, ws, &case.id).await?.unwrap_or(case))
}

/// The insight service's answer, in this service's error type (the two share the same wording).
fn insight_err(e: crate::insight::InsightSvcError) -> CaseSvcError {
    match e {
        crate::insight::InsightSvcError::BadInput(m) => CaseSvcError::BadInput(m),
        crate::insight::InsightSvcError::Denied => CaseSvcError::Denied,
        other => CaseSvcError::Store(other.to_string()),
    }
}
