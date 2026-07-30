//! Subject resolution for the triage plane — "is this a legal assignee here?" and "who is *me*?"
//! (insight-triage-scope.md, resolved decision 2).
//!
//! Two reads the `lb_insights` crate is deliberately agnostic of (membership + the team graph live
//! in `lb_authz`/`lb_assets`), kept in one file so the workspace-wall argument is stated once.
//!
//! **The opacity rule is the security-relevant part.** `validate_assignee` returns the SAME error
//! for a subject that does not exist, a user who is not a member of this workspace, and a real
//! member of *another* workspace. All three reads are workspace-scoped, so the third case is
//! structurally indistinguishable from the first — a probe cannot learn that `user:ada` exists in
//! ws-B by trying to assign to them in ws-A. Do not "improve" this by naming which case failed.

use lb_assets::list_related;
use lb_authz::{membership_is_member, team_list, MEMBER};
use lb_insights::{OwnerSubjects, Subscription, SUB_ASSIGNEE_ME};
use lb_store::Store;

use super::error::InsightSvcError;

/// The one refusal message for every illegal assignee. Deliberately says nothing about WHICH check
/// failed — see the module doc.
const NOT_A_MEMBER: &str =
    "assignee is not a member of this workspace — assign to a `user:` who has joined or a `team:` \
     that exists here";

/// Refuse `assignee` unless it is a live member (`user:…`) or an existing team (`team:…`) of `ws`.
///
/// Assigning to a subject that cannot read the insight is never intentional, and one cheap
/// membership read on a low-frequency verb is the whole cost. Both reads are ws-namespaced, so this
/// also *is* the no-cross-workspace-assignment rule (a ws-B member is simply absent here).
pub async fn validate_assignee(
    store: &Store,
    ws: &str,
    assignee: &str,
) -> Result<(), InsightSvcError> {
    let ok = match assignee.split_once(':') {
        // A team is legal from v1 — queue-style ownership ("the mechanical crew owns this") is how
        // real triage works, and retrofitting `team:` later would break every consumer that had
        // parsed `assigned_to` as a user sub (resolved decision 2).
        Some(("team", name)) if !name.is_empty() => team_list(store, ws)
            .await
            .map_err(|e| InsightSvcError::Store(e.to_string()))?
            .iter()
            .any(|t| t.team == assignee || t.team == name),
        Some(("user", name)) if !name.is_empty() => membership_is_member(store, ws, assignee)
            .await
            .map_err(|e| InsightSvcError::Store(e.to_string()))?,
        // Anything else (`key:`, `ext:`, a bare string, an empty kind) is not a person or a queue
        // and cannot own a finding.
        _ => false,
    };
    if !ok {
        return Err(InsightSvcError::BadInput(NOT_A_MEMBER.into()));
    }
    Ok(())
}

/// Every subject the principal counts as for the `assigned_to: "me"` roster view — their own `sub`
/// plus each team they belong to.
///
/// The teams half is load-bearing and easy to miss: a naive `assigned_to == principal.sub()` check
/// silently drops every insight assigned to a *queue* the caller is on, so "my work" would hide
/// exactly the team-owned findings the team-subject decision exists to support.
pub async fn me_subjects(store: &Store, ws: &str, sub: &str) -> Vec<String> {
    let mut subjects = vec![sub.to_string()];
    // Best-effort: a team-graph hiccup narrows the view to the caller's own sub rather than failing
    // the roster read. Logged, because a silently narrowed "mine" looks like "nothing assigned".
    match team_list(store, ws).await {
        Ok(teams) => {
            for team in teams {
                match list_related(store, ws, MEMBER, &team.team).await {
                    Ok(members) if members.iter().any(|m| m == sub) => {
                        subjects.push(if team.team.starts_with("team:") {
                            team.team.clone()
                        } else {
                            format!("team:{}", team.team)
                        });
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!(ws, team = %team.team, error = ?e,
                        "insight assignee: team membership unreadable; 'me' view narrowed"),
                }
            }
        }
        Err(e) => tracing::warn!(ws, error = ?e,
            "insight assignee: team list unreadable; 'me' view narrowed to the caller's own sub"),
    }
    subjects
}

/// Build the [`OwnerSubjects`] map the matcher needs to resolve `assignee: "me"` — one entry per
/// **distinct owner** of a subscription that actually uses `"me"`.
///
/// Deliberately lazy on two axes, because this runs on the raise *write* path: subs that name a
/// concrete subject (or no assignee at all) need no expansion, and each owner is resolved once no
/// matter how many subs they own. A workspace where nobody uses `"me"` does **zero** extra reads.
///
/// Resolved per call rather than cached across calls — see [`me_subjects`]; a cache that outlives a
/// team change makes a subscription silently stop matching.
// SCOPE: docs/scope/insights/insight-assignee-notify-scope.md §"Risks" (the per-fire read)
pub async fn owner_subjects_for(store: &Store, ws: &str, subs: &[Subscription]) -> OwnerSubjects {
    let mut map = OwnerSubjects::new();
    for sub in subs {
        if sub.filter.assignee.as_deref() != Some(SUB_ASSIGNEE_ME) {
            continue;
        }
        if map.contains_key(&sub.owner) {
            continue;
        }
        let subjects = me_subjects(store, ws, &sub.owner).await;
        map.insert(sub.owner.clone(), subjects.into_iter().collect());
    }
    map
}
