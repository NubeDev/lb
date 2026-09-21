//! `case.assignees` — the subjects this caller may hand work to (case-plane scope §4a).
//!
//! The assign control had no picker: it could offer "assign to me" and a free-text box, and nothing
//! else, because every verb that enumerates people is admin-gated (`membership.list` →
//! `members.manage`, `identity.list` → `identity.manage`, `teams.list` → `teams.list`). The queue
//! itself is a **viewer** surface, so the one screen that needs a roster was the one screen that
//! could not read one.
//!
//! This verb closes that without minting a new disclosure, because the workspace-wide roster is not
//! what a picker needs. It answers the narrower question — *who do I share a team with?* — and every
//! row it returns is already readable by the same caller through grants they hold today:
//!
//!   - the caller's own sub, and the teams they are on: exactly what [`super::super::insight::
//!     assignee::me_subjects`] already computes on every `case.list?lane=mine`, under only the
//!     `case.list` gate. Same data, same altitude, already shipped.
//!   - the members of those teams: `members.list(team)` is `mcp:members.list:call`, which lb puts in
//!     `VIEWER_CAPS` ("list is viewer; add/manage are admin"). A viewer may already read this one
//!     team at a time; this verb saves them the round trips, it does not widen the wall.
//!
//! **The privacy line, stated so it can be tested.** A caller must NOT learn from this verb:
//!   - that a team they are not on exists — only walked teams they are a member of are emitted, so
//!     this is never a back door onto admin `teams.list`;
//!   - any member who shares no team with them — that is `membership.list`'s disclosure and it stays
//!     behind `members.manage`;
//!   - any email or display identity — subjects only, because `identity.list` is admin;
//!   - anything from another workspace — all three reads are ws-namespaced.
//!
//! `a_member_learns_nothing_about_teams_they_are_not_on` is the test that holds this line; it fails
//! the day somebody "simplifies" the walk into `membership_list` + `team_list`.
//!
//! **This is a SUGGESTION LIST, not an allow-list.** `validate_assignee` accepts any live member of
//! the workspace and any team that exists — a strictly wider set than this returns, and deliberately
//! so, because assigning across teams is a normal operational act. Do not "tighten" the write to
//! this list: that would silently break cross-team assignment. The free-text box stays, below the
//! picker.
//!
//! **Gate: `case.list`, via the alias in `tool_gate.rs`.** Not its own name — a `mcp:case.assignees:
//! call` cap exists in no role bundle, so deriving one by convention would make the verb `Denied`
//! for every caller including admins. The inner `authorize_tool` below therefore asks for
//! `case.list` too: strictest wins, so an inner check on its own name would deny straight through an
//! otherwise-correct alias.
//!
//! **No paging, on purpose.** The bound is "people who share a team with you"; a workspace where
//! that overflows a picker has a problem paging would only hide. The walk is a `team_list` scan plus
//! one `list_related` per team — the same unindexed cost `me_subjects` already pays on every
//! `Mine`/`Watching` list, so the marginal cost of opening a popover is nil. There is no cache, for
//! the reason `assignee.rs` already records: one that outlives a team change makes the picker
//! silently wrong.

use lb_auth::Principal;
use lb_authz::{membership_is_member, team_list};
use lb_mcp::authorize_tool;
use lb_store::Store;
use serde::{Deserialize, Serialize};

use super::error::CaseSvcError;
use crate::insight::team_member_edges;

/// One team the caller is on. The display `name` rides along because a picker that prints
/// `team:mechanical` where the workspace says "Mechanical crew" is worse than one that does not —
/// and the name of a team you are yourself a member of is not a secret.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssigneeTeam {
    /// The `team:`-prefixed subject, in the form `case.assign` accepts.
    pub team: String,
    /// The workspace's display name for it (may be empty if it was never set).
    pub name: String,
}

/// The subjects this caller may hand work to.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Assignees {
    /// The caller's own subject — what *Assign to me* sends.
    pub me: String,
    /// The teams the caller belongs to, sorted by subject.
    pub teams: Vec<AssigneeTeam>,
    /// The `user:` subjects who share at least one team with the caller, sorted, `me` excluded (the
    /// picker offers *Assign to me* separately and a duplicate row is noise).
    pub users: Vec<String>,
}

/// Resolve the caller's own subject, their teams, and their teammates in ONE walk of the team graph.
///
/// Deliberately not `me_subjects` + a second pass: one traversal produces both halves, and a second
/// walk in a second file is what `case/list.rs` argues against.
///
/// Best-effort on the graph, matching `me_subjects`: a team whose edges are unreadable is warned and
/// skipped rather than failing the read, because a narrowed picker beats a broken one — and the
/// free-text box is always there as the exit.
pub async fn case_assignees(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Assignees, CaseSvcError> {
    authorize_tool(principal, ws, "case.list").map_err(|_| CaseSvcError::Denied)?;

    let sub = principal.sub();
    let mut out = Assignees {
        me: sub.to_string(),
        ..Default::default()
    };

    let teams = match team_list(store, ws).await {
        Ok(teams) => teams,
        // Same posture as `me_subjects`: log and degrade to "just me" rather than fail the popover.
        Err(e) => {
            tracing::warn!(ws, error = ?e,
                "case assignees: team list unreadable; picker narrowed to the caller");
            return Ok(out);
        }
    };

    for team in teams {
        // Both spellings of the team id — see `team_member_edges`. The picker shares the `Mine`
        // lane's resolution because they must agree on what "my team" means; two walks that
        // disagreed would offer a team the lane then showed nothing for.
        let members = match team_member_edges(store, ws, &team.team).await {
            Ok(members) => members,
            Err(e) => {
                tracing::warn!(ws, team = %team.team, error = ?e,
                    "case assignees: team membership unreadable; team omitted from the picker");
                continue;
            }
        };
        // The membership check IS the privacy line: a team the caller is not on contributes neither
        // its name nor its members, so this verb never discloses what admin `teams.list` would.
        if !members.iter().any(|m| m == sub) {
            continue;
        }
        // `team_list` may yield a bare id; `case.assign` accepts both forms but the picker should
        // send the prefixed one, exactly as `me_subjects` normalises it.
        let subject = if team.team.starts_with("team:") {
            team.team.clone()
        } else {
            format!("team:{}", team.team)
        };
        out.teams.push(AssigneeTeam {
            team: subject,
            name: team.name.clone(),
        });
        out.users.extend(
            members
                .into_iter()
                .filter(|m| m != sub && m.starts_with("user:")),
        );
    }

    // `list_related` order is explicitly unspecified, so sort for a stable render; a team the caller
    // shares twice over must not appear twice.
    out.teams.sort_by(|a, b| a.team.cmp(&b.team));
    out.teams.dedup_by(|a, b| a.team == b.team);
    out.users.sort();
    out.users.dedup();

    // **Only offer what `case.assign` will accept.** A `member` edge is an unvalidated write: it can
    // name a subject who never joined this workspace (or who has since been removed), while
    // `validate_assignee` requires a LIVE workspace membership. Without this filter the picker
    // offers a row that the assign it exists to perform then refuses with "assignee is not a member
    // of this workspace" — the control contradicting itself, which reads as a broken assign rather
    // than a stale roster.
    //
    // Filtered here, after the dedup, so each distinct subject costs one membership read no matter
    // how many teams it shares with the caller. A read that FAILS drops the row: a picker that
    // silently offers less is the same degrade posture as the unreadable-team arm above, and the
    // typed box remains the door to anything omitted.
    //
    // Teams are NOT filtered this way — `validate_assignee` accepts any team that exists, and the
    // team was just read out of `team_list`, so it validates by construction.
    let mut live = Vec::with_capacity(out.users.len());
    for user in out.users {
        match membership_is_member(store, ws, &user).await {
            Ok(true) => live.push(user),
            Ok(false) => tracing::debug!(ws, user = %user,
                "case assignees: team member is not a workspace member; omitted from the picker"),
            Err(e) => tracing::warn!(ws, user = %user, error = ?e,
                "case assignees: membership unreadable; user omitted from the picker"),
        }
    }
    out.users = live;
    Ok(out)
}
