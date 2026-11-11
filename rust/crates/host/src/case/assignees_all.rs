//! The WIDE assign picker: every team and every live member of the workspace, for a caller who holds
//! `mcp:case.assignees.all:call`. One responsibility: answer the roster question without the team
//! walk, for the callers a workspace has chosen to trust with the whole roster.
//!
//! Why a cap and not a bigger default: `case.assignees` answers "who do I share a team with" because
//! the workspace roster is an admin-gated disclosure (`membership.list` → `members.manage`). Some
//! operators triage for the WHOLE workspace (a service desk that hands cases to any site team) and
//! for them the narrow list means typing addresses into the free-text box all day. This cap lets a
//! workspace grant the wide list to exactly that group, usually through a team grant, while every
//! other caller keeps the narrow one.
//!
//! The cap sits in the admin bundle only: an admin can already read the roster through
//! `membership.list`, so nothing new is disclosed to them. It is in no member or viewer bundle,
//! because handing every member the roster is the trade `assignees.rs` refuses.
//!
//! The payload shape is the narrow verb's, so the picker renders both without knowing which it got.

use lb_authz::{membership_list, team_list};
use lb_store::Store;

use super::assignees::{AssigneeTeam, Assignees};

/// The `<ext>.<tool>` resource the wide picker is gated on: `mcp:case.assignees.all:call`.
pub(super) const ASSIGNEES_ALL: &str = "case.assignees.all";

/// Every team and every live `user:` member of `ws`, the caller excluded from `users` (the picker
/// offers *Assign to me* separately). `None` when either read fails, so the caller can fall back to
/// the narrow walk: a picker that offers less beats one that fails, the posture `assignees.rs` takes.
pub(super) async fn whole_workspace(store: &Store, ws: &str, me: &str) -> Option<Assignees> {
    let teams = match team_list(store, ws).await {
        Ok(teams) => teams,
        Err(e) => {
            tracing::warn!(ws, error = ?e, "case assignees (all): team list unreadable; narrow picker");
            return None;
        }
    };
    let members = match membership_list(store, ws).await {
        Ok(members) => members,
        Err(e) => {
            tracing::warn!(ws, error = ?e, "case assignees (all): roster unreadable; narrow picker");
            return None;
        }
    };

    // `team_list` may yield a bare id; send the prefixed form `case.assign` resolves, as the narrow
    // walk does. Every team here exists, so `validate_assignee` accepts it by construction.
    let mut out_teams: Vec<AssigneeTeam> = teams
        .into_iter()
        .map(|t| AssigneeTeam {
            team: if t.team.starts_with("team:") {
                t.team
            } else {
                format!("team:{}", t.team)
            },
            name: t.name,
        })
        .collect();
    out_teams.sort_by(|a, b| a.team.cmp(&b.team));
    out_teams.dedup_by(|a, b| a.team == b.team);

    // `membership_list` returns LIVE memberships only, so every row is one `case.assign` accepts.
    let mut users: Vec<String> = members
        .into_iter()
        .map(|m| m.sub)
        .filter(|s| s != me && s.starts_with("user:"))
        .collect();
    users.sort();
    users.dedup();

    Some(Assignees {
        me: me.to_string(),
        teams: out_teams,
        users,
    })
}
