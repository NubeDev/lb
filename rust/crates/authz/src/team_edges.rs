//! Team membership as the resolvers read it — one place for the two spelling traps (lb#219).
//!
//! **Why this exists.** A team record stores whatever id it was created with, verbatim
//! (`team_create` normalises nothing), so `mechanical` and `team:mechanical` are both real ids. The
//! `member` edge is keyed by that raw id and stores whatever user string the caller sent — the admin
//! console sends `user:<name>`. The cap resolvers, meanwhile, are handed the BARE user name (the
//! session mint and `reminder/fire.rs` strip `user:` because direct grants are stored under it).
//! Comparing a bare name to a prefixed edge never matched, so every grant and role given to a team
//! silently reached none of its members. And a team created as `team:x` was folded as
//! `Subject::Team("team:x")`, whose key is `team:team:x` — a key no grant is ever written under.
//!
//! Every membership read in the resolvers goes through here so the three of them (flat, scoped,
//! sourced) cannot drift apart again.

use lb_assets::list_related;
use lb_store::{Store, StoreError};

use crate::subject::Subject;
use crate::MEMBER;

/// The `member` edges of `team`, looked up under BOTH spellings of the team id. The other spelling
/// is read only when the first finds nothing, so a correctly-keyed team costs one read.
pub async fn team_member_edges(
    store: &Store,
    ws: &str,
    team: &str,
) -> Result<Vec<String>, StoreError> {
    let mut members = list_related(store, ws, MEMBER, team).await?;
    if members.is_empty() {
        let other = match team.strip_prefix("team:") {
            Some(bare) => bare.to_string(),
            None => format!("team:{team}"),
        };
        members = list_related(store, ws, MEMBER, &other).await?;
    }
    Ok(members)
}

/// Does membership `edge` name `user`? Both sides are compared without the `user:` prefix, so a bare
/// or prefixed edge matches a bare or prefixed user.
pub fn edge_is_user(edge: &str, user: &str) -> bool {
    bare_user(edge) == bare_user(user)
}

/// The grant subject for team id `team`, with any `team:` prefix removed so its key is `team:<name>`
/// whichever spelling the team was created with.
pub fn team_subject(team: &str) -> Subject {
    Subject::Team(team.strip_prefix("team:").unwrap_or(team).to_string())
}

fn bare_user(s: &str) -> &str {
    s.strip_prefix("user:").unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_member_edge_matches_the_user_under_either_spelling() {
        assert!(edge_is_user("user:priya", "priya"));
        assert!(edge_is_user("priya", "user:priya"));
        assert!(edge_is_user("user:priya", "user:priya"));
        assert!(edge_is_user("priya", "priya"));
        assert!(!edge_is_user("user:priya", "bob"));
        // A prefix is stripped once, never used to fold two different names together.
        assert!(!edge_is_user("user:user:priya", "priya"));
    }

    #[test]
    fn a_team_subject_never_doubles_the_prefix() {
        assert_eq!(team_subject("mechanical").as_key(), "team:mechanical");
        assert_eq!(team_subject("team:mechanical").as_key(), "team:mechanical");
    }
}
