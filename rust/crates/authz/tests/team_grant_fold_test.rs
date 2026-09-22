//! lb#219: a grant given to a TEAM must reach the team's members, whichever way the membership edge
//! and the team id are spelled. The admin console stores member edges as `user:<name>` while the
//! resolvers are handed the bare name, and teams can be created as `x` or `team:x`; before the fix
//! the fold compared the two spellings and every team grant silently reached nobody. Real store,
//! seeded through the real write path; all three resolvers (flat, scoped, sourced) are pinned so
//! they cannot drift apart again.

use lb_assets::relate;
use lb_authz::{
    grant_assign, resolve_caps, resolve_caps_scoped, resolve_caps_sourced, team_create, Subject,
    MEMBER,
};
use lb_store::Store;

const WS: &str = "nube";
const CAP: &str = "mcp:case.workflow:call";

/// A workspace where team `team_id` holds `CAP` and `user` is a member via an edge written as
/// `edge`. The grant is written the way `grants.assign {subject: "team:<name>"}` writes it.
async fn seed(team_id: &str, edge: &str) -> Store {
    let store = Store::memory().await.unwrap();
    team_create(&store, WS, team_id, "Viewers").await.unwrap();
    let grant_subject = Subject::parse(&format!(
        "team:{}",
        team_id.strip_prefix("team:").unwrap_or(team_id)
    ))
    .unwrap();
    grant_assign(&store, WS, &grant_subject, CAP).await.unwrap();
    relate(&store, WS, MEMBER, team_id, edge).await.unwrap();
    store
}

async fn all_three_hold(store: &Store, user: &str) -> (bool, bool, bool) {
    let flat = resolve_caps(store, WS, user).await.unwrap();
    let scoped = resolve_caps_scoped(store, WS, user).await.unwrap();
    let sourced = resolve_caps_sourced(store, WS, user).await.unwrap();
    (
        flat.iter().any(|c| c == CAP),
        scoped.iter().any(|c| c.cap == CAP),
        sourced.iter().any(|c| c.cap == CAP),
    )
}

#[tokio::test]
async fn a_team_grant_reaches_a_member_whose_edge_is_prefixed() {
    // The shape the admin console writes: `user:<name>` edge, bare team id. This is the live bug.
    let store = seed("viewers", "user:priya").await;
    assert_eq!(all_three_hold(&store, "priya").await, (true, true, true));
}

#[tokio::test]
async fn a_team_grant_reaches_a_member_whose_edge_is_bare() {
    let store = seed("viewers", "priya").await;
    assert_eq!(all_three_hold(&store, "priya").await, (true, true, true));
}

#[tokio::test]
async fn a_team_created_with_the_prefix_still_folds_its_grant() {
    // `team:viewers` as the stored id must fold as `team:viewers`, never `team:team:viewers`.
    let store = seed("team:viewers", "user:priya").await;
    assert_eq!(all_three_hold(&store, "priya").await, (true, true, true));
}

#[tokio::test]
async fn a_non_member_still_gets_nothing_from_the_team() {
    let store = seed("viewers", "user:priya").await;
    assert_eq!(all_three_hold(&store, "bob").await, (false, false, false));
}
