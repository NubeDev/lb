//! `case_list` — the three work lanes over the capability gate (case-plane scope).
//!
//! The one thing this layer adds that the crate cannot: **who "me" is.** The [`lb_cases::Lane::Mine`]
//! lane is "the person plus every team they belong to", and that expansion needs the membership and
//! team planes `lb-cases` is deliberately agnostic of.
//!
//! It reuses `insight/assignee.rs::me_subjects` — the SAME function the shipped
//! `SubFilter.assignee: "me"` semantics resolve through (`lb_insights::OwnerSubjects` /
//! `owner_subjects_for`). One definition of "mine", used by the subscription matcher, the insight
//! roster and now the case lanes. A second definition here would be how "my work" silently stops
//! showing the team-owned jobs the `team:` subject decision exists to support.

use lb_auth::Principal;
use lb_cases::{Lane, ListPage, ListQuery};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::CaseSvcError;
use crate::insight::me_subjects;

/// List cases in `ws` matching `query`, `due_at` ascending then severity descending. Gated by
/// `mcp:case.list:call`.
pub async fn case_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
    query: ListQuery,
) -> Result<ListPage, CaseSvcError> {
    authorize_tool(principal, ws, "case.list").map_err(|_| CaseSvcError::Denied)?;
    // Costs nothing on the lanes that do not need it: only `Mine` and `Watching` are defined
    // relative to the caller, and `Waiting` is a property of the case alone.
    let subjects = match query.lane {
        Lane::Mine | Lane::Watching => me_subjects(store, ws, principal.sub())
            .await
            .into_iter()
            .collect(),
        Lane::Waiting => Default::default(),
    };
    // Entity-scoped data: a restricted caller sees only cases at their sites. Set here from server
    // state; the wire can never carry it (`sites_allowed` is `serde(skip)`).
    let mut query = query;
    if let Some((_tag, ids)) = crate::insight::entity_limit(store, principal, ws)
        .await
        .map_err(|e| CaseSvcError::Store(e.to_string()))?
    {
        query.filter.sites_allowed = Some(ids);
    }
    Ok(lb_cases::list(store, ws, &query, &subjects).await?)
}
