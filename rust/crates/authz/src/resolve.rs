//! [`resolve_caps`] — the session projection: compute the capability set a login token should carry
//! for `user` in workspace `ws` (authz-grants scope). The token is a **cached projection** of this;
//! the enforcement gates (`lb_caps::check`) read the token, not this function, on the hot path.
//!
//! ```text
//!   caps = ( direct user grants
//!          ∪ the user's roles' caps
//!          ∪ for each team the user is a member of:  team's grants ∪ team's roles' caps )
//! ```
//!
//! **The freshness asymmetry (load-bearing).** Because the token caches this, a Gate-2 capability
//! change (grant/revoke a cap or role, add/remove a *team-inherited* cap) only takes effect on the
//! next **re-mint** — bounded by the token TTL. A Gate-3 membership/visibility change is **live**
//! (the S4 relation is re-resolved per read). So "remove Bob from the team" drops his access to
//! resources shared to that team *immediately* (Gate 3) but leaves team-inherited caps in his
//! current token until it expires. This function is the Gate-2 (cached) half; do not assume
//! revocation is uniformly instant. See the resolver's tests and the admin-crud revoke seam.

use std::collections::BTreeSet;

use lb_assets::list_related;
use lb_store::{Store, StoreError};

use crate::grant::grant_list;
use crate::role::role_caps;
use crate::subject::Subject;
use crate::team::team_list;
use crate::MEMBER;

/// Resolve `user`'s effective caps in workspace `ws` as the union described above. Deduplicated and
/// sorted (a `BTreeSet`) so the minted token is deterministic (testing §3 — no incidental ordering).
pub async fn resolve_caps(store: &Store, ws: &str, user: &str) -> Result<Vec<String>, StoreError> {
    let mut caps: BTreeSet<String> = BTreeSet::new();

    // Direct user grants (+ any roles they name).
    resolve_subject_caps(store, ws, &Subject::User(user.to_string()), &mut caps).await?;

    // Team-inherited: for every team the user is a member of, fold the team's grants + roles.
    for team in team_list(store, ws).await? {
        let members = list_related(store, ws, MEMBER, &team.team).await?;
        if members.iter().any(|m| m == user) {
            resolve_subject_caps(store, ws, &Subject::Team(team.team.clone()), &mut caps).await?;
        }
    }

    Ok(caps.into_iter().collect())
}

/// Resolve an arbitrary `subject`'s **direct** caps in workspace `ws` — direct grants plus the
/// expansion of any `role:<name>` grant into that role's bundled caps — into `caps` (api-keys
/// scope). This is the generalized inner of [`resolve_caps`]: a user folds this for itself AND its
/// teams (team membership is a *user* concept); an API key (`Subject::Key`) calls this directly — it
/// has direct grants + roles but **no team-membership edge** (a key joins no teams in v1).
///
/// **Why a key must call THIS and not `resolve_caps(&str)`:** `resolve_caps` wraps its `user` arg in
/// `Subject::User`, so passing `"key:…"` would build `Subject::User("key:…")` → resolve to **zero**
/// caps → silently deny everything. A key passes its own `Subject::Key(id)` here.
pub async fn resolve_subject_caps(
    store: &Store,
    ws: &str,
    subject: &Subject,
    caps: &mut BTreeSet<String>,
) -> Result<(), StoreError> {
    for cap in grant_list(store, ws, subject).await? {
        match cap.strip_prefix("role:") {
            Some(role) => {
                for rc in role_caps(store, ws, role).await? {
                    caps.insert(rc);
                }
            }
            None => {
                caps.insert(cap);
            }
        }
    }
    Ok(())
}
