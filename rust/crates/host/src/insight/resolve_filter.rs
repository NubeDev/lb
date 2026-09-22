//! Resolve the two filter axes the insights crate cannot resolve itself — the tag facet and the
//! owner — into the pre-resolved arguments `lb_insights::list` and `lb_insights::count` both take.
//!
//! One file, one resolution. Every reader of a filter MUST agree about what it
//! selects: a tile reading "78 open" beside a roster showing a different set is worse than a slow
//! tile, because nothing on screen says which is lying. Two copies of "expand `me` into the caller
//! plus their teams" is precisely how that drift starts, so there is only one.
//!
//! Both axes are host concerns by construction: the tag graph lives in `lb_tags` and team
//! membership in `lb_authz`/`lb_assets`, and the insights crate is deliberately agnostic of both
//! (README §7).

use std::collections::HashSet;

use lb_auth::Principal;
use lb_insights::{AssigneeFilter, ListFilter, ASSIGNEE_ME, ASSIGNEE_NONE};
use lb_store::Store;
use lb_tags::Facet;

use super::assignee::me_subjects;
use super::error::InsightSvcError;

/// The host-resolved halves of a filter: the tag allowlist (`None` ⇒ no tag facet) and the owner
/// filter (`None` ⇒ no owner axis).
pub(super) type ResolvedFilter = (Option<HashSet<String>>, Option<AssigneeFilter>);

/// Resolve `filter`'s tag facet and owner axis against the caller.
///
/// The `insight.*` capability the caller already passed authorizes this workspace read; a tag facet
/// is a filter over already-authorized insights, not a new privilege, so the raw graph read is
/// correct here.
pub(super) async fn resolve_filter(
    store: &Store,
    principal: &Principal,
    ws: &str,
    filter: &ListFilter,
) -> Result<ResolvedFilter, InsightSvcError> {
    let tag_allow: Option<HashSet<String>> = if filter.tags.is_empty() {
        None
    } else {
        let facets: Vec<Facet> = filter
            .tags
            .iter()
            .map(|(k, v)| Facet::exact(k.clone(), serde_json::Value::String(v.clone())))
            .collect();
        let entities = lb_tags::find(store, ws, &facets)
            .await
            .map_err(|e| InsightSvcError::Store(e.to_string()))?;
        // Entities come back as `insight:<id>` refs; both crate verbs match on the bare id.
        Some(
            entities
                .into_iter()
                .map(|e| e.strip_prefix("insight:").map(str::to_string).unwrap_or(e))
                .collect(),
        )
    };

    // Entity-scoped data: a restricted caller's view is ALSO limited to their entities' insights —
    // intersected with any tag facet, so a facet can only ever narrow further.
    let tag_allow = match super::entity_filter::allowed_ids(store, principal, ws).await? {
        None => tag_allow,
        Some(scope) => Some(match tag_allow {
            None => scope,
            Some(facet) => facet.intersection(&scope).cloned().collect(),
        }),
    };

    let assignee = match filter.assigned_to.as_deref() {
        None => None,
        Some(ASSIGNEE_NONE) => Some(AssigneeFilter::Unassigned),
        Some(ASSIGNEE_ME) => Some(AssigneeFilter::AnyOf(
            me_subjects(store, ws, principal.sub())
                .await
                .into_iter()
                .collect(),
        )),
        // An explicit subject. NOT membership-validated: filtering by a subject who has since left
        // is exactly how an admin FINDS the orphaned queue a resolved assignment leaves behind, and
        // a filter reveals nothing a full roster read would not.
        Some(subject) => Some(AssigneeFilter::AnyOf(
            [subject.to_string()].into_iter().collect(),
        )),
    };

    Ok((tag_allow, assignee))
}
