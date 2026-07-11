//! [`resolve_caps_scoped`] — the entity-scoped twin of [`resolve_caps`](crate::resolve_caps). Same
//! fold (direct ∪ roles ∪ team-inherited), but instead of collapsing each cap into a flat set it
//! carries the **scope union** per cap (entity-scoped-grants scope). A principal holding the same
//! cap through several scoped grants gets the union of selectors; any `All` grant wins.
//!
//! Role-expanded caps are always `All` scope — a role defines *what you can do*, not *which
//! records*. Only direct grants carry a scope. This matches the scope doc: "a grant carries a
//! resource selector" — the role is a cap bundle, not a scoped entity.
//!
//! **Not a parallel resolver.** This runs the same `grant_list_scoped` → role-expand loop as
//! `resolve_subject_caps_with`, just accumulating `Scope` alongside each cap. The cap *set* it
//! yields equals `resolve_caps(store, ws, user)` — there is no resolver drift.

use std::collections::BTreeMap;

use lb_assets::list_related;
use lb_store::{Store, StoreError};
use serde::{Deserialize, Serialize};

use crate::grant::grant_list_scoped;
use crate::resolve::BuiltinRoleCaps;
use crate::role::role_caps;
use crate::scope::Scope;
use crate::subject::Subject;
use crate::team::team_list;
use crate::MEMBER;

/// One resolved cap plus its unioned scope (entity-scoped-grants scope). `scope: All` means the
/// cap is fully reachable (either an `All` grant or a role expansion); `scope: Ids` means the cap
/// is narrowed to those ids in that table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopedCap {
    pub cap: String,
    pub scope: Scope,
}

/// Resolve `user`'s effective caps WITH scope unions — the scoped twin of
/// [`resolve_caps`](crate::resolve_caps). Same fold, each cap carrying the union of all scopes it
/// was granted through. Any `All` grant makes the cap `All`. Reads only stored role records (no
/// live built-in caps); the host entry passes `LiveBuiltinRoleCaps` via
/// [`resolve_caps_scoped_with`].
pub async fn resolve_caps_scoped(
    store: &Store,
    ws: &str,
    user: &str,
) -> Result<Vec<ScopedCap>, StoreError> {
    resolve_caps_scoped_with(store, ws, user, &crate::resolve::NoBuiltinRoleCaps).await
}

/// [`resolve_caps_scoped`] with an injected [`BuiltinRoleCaps`] (builtin-role-freshness scope).
pub async fn resolve_caps_scoped_with(
    store: &Store,
    ws: &str,
    user: &str,
    builtins: &dyn BuiltinRoleCaps,
) -> Result<Vec<ScopedCap>, StoreError> {
    let mut acc: BTreeMap<String, Scope> = BTreeMap::new();
    fold_subject_scoped(
        store,
        ws,
        &Subject::User(user.to_string()),
        builtins,
        &mut acc,
    )
    .await?;
    for team in team_list(store, ws).await? {
        let members = list_related(store, ws, MEMBER, &team.team).await?;
        if members.iter().any(|m| m == user) {
            fold_subject_scoped(
                store,
                ws,
                &Subject::Team(team.team.clone()),
                builtins,
                &mut acc,
            )
            .await?;
        }
    }
    Ok(acc
        .into_iter()
        .map(|(cap, scope)| ScopedCap { cap, scope })
        .collect())
}

/// Fold `subject`'s grants into `acc`, carrying scope. Role-expanded caps are `All`; direct grants
/// carry their scope. If the same cap is contributed multiple times, the scopes union (any `All`
/// wins).
async fn fold_subject_scoped(
    store: &Store,
    ws: &str,
    subject: &Subject,
    builtins: &dyn BuiltinRoleCaps,
    acc: &mut BTreeMap<String, Scope>,
) -> Result<(), StoreError> {
    for grant in grant_list_scoped(store, ws, subject).await? {
        match grant.cap.strip_prefix("role:") {
            Some(role) => {
                // Role caps are All scope (a role defines what you can do, not which records).
                for rc in role_caps(store, ws, role).await? {
                    union_into(acc, rc, Scope::All);
                }
                if let Some(live) = builtins.live_caps(role) {
                    for rc in live {
                        union_into(acc, rc, Scope::All);
                    }
                }
            }
            None => {
                union_into(acc, grant.cap, grant.scope);
            }
        }
    }
    Ok(())
}

/// Union `scope` into the existing scope for `cap` in `acc` (or insert if absent).
fn union_into(acc: &mut BTreeMap<String, Scope>, cap: String, scope: Scope) {
    match acc.get_mut(&cap) {
        Some(existing) => *existing = existing.union(&scope),
        None => {
            acc.insert(cap, scope);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_into_all_wins() {
        let mut acc: BTreeMap<String, Scope> = BTreeMap::new();
        union_into(
            &mut acc,
            "cap1".into(),
            Scope::Ids {
                table: "t".into(),
                ids: vec!["a".into()],
            },
        );
        union_into(&mut acc, "cap1".into(), Scope::All);
        assert_eq!(acc.get("cap1"), Some(&Scope::All));
    }

    #[test]
    fn union_into_ids_merge() {
        let mut acc: BTreeMap<String, Scope> = BTreeMap::new();
        union_into(
            &mut acc,
            "cap1".into(),
            Scope::Ids {
                table: "t".into(),
                ids: vec!["a".into()],
            },
        );
        union_into(
            &mut acc,
            "cap1".into(),
            Scope::Ids {
                table: "t".into(),
                ids: vec!["b".into()],
            },
        );
        match acc.get("cap1") {
            Some(Scope::Ids { table, ids }) => {
                assert_eq!(table, "t");
                assert_eq!(ids.len(), 2);
            }
            _ => panic!("expected Ids"),
        }
    }
}
