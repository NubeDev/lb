//! `membership.list` — the workspace's effective roster (global-identity scope, decision #9: the
//! People-tab source). Gated by `mcp:members.manage:call`, workspace-first. Returns **effective
//! members** = membership rows ∪ legacy `user:*` rows (lazy migration, decision #10), so an upgraded
//! workspace's existing users still appear and "no access gained or lost" holds. Each row carries the
//! identity's display name when one is resolved. Lazy-creates `identity:{sub}` on first touch.

use lb_auth::Principal;
use lb_authz as raw;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::MembershipError;
use super::model::MembershipView;

/// The effective members of `ws` as `principal` sees them, sorted by `sub`.
pub async fn membership_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<MembershipView>, MembershipError> {
    authorize_tool(principal, ws, "members.manage").map_err(|_| MembershipError::Denied)?;
    let mut seen: std::collections::BTreeMap<String, MembershipView> =
        std::collections::BTreeMap::new();
    // 1. Explicit membership rows.
    for m in raw::membership_list(store, ws).await? {
        ensure_identity(store, &m.sub).await;
        let display = raw::identity_get(store, &m.sub)
            .await?
            .and_then(|i| i.display_name);
        seen.entry(m.sub.clone()).or_insert(MembershipView {
            sub: m.sub,
            joined_ts: m.joined_ts,
            display_name: display,
        });
    }
    // 2. Legacy user rows (lazy migration) — a `user:<sub>` admin record (not tombstoned) is an
    //    implicit membership, so the roster does not lose existing users on upgrade.
    let rows = lb_store::list(store, ws, crate::users::TABLE, "kind", crate::users::KIND).await?;
    for v in rows {
        if v.get("kind").and_then(|k| k.as_str()) == Some(crate::users::TOMBSTONE) {
            continue;
        }
        let sub = match v.get("user").and_then(|u| u.as_str()) {
            Some(u) => format!("user:{u}"),
            None => continue,
        };
        let joined_ts = v.get("ts").and_then(|t| t.as_u64()).unwrap_or(0);
        ensure_identity(store, &sub).await;
        let display = raw::identity_get(store, &sub)
            .await?
            .and_then(|i| i.display_name);
        seen.entry(sub.clone()).or_insert(MembershipView {
            sub,
            joined_ts,
            display_name: display,
        });
    }
    Ok(seen.into_values().collect())
}

/// Lazy-create `identity:{sub}` if absent (decision #10) — best-effort; a write error never breaks
/// the list.
async fn ensure_identity(store: &Store, sub: &str) {
    if raw::identity_get(store, sub)
        .await
        .map(|o| o.is_none())
        .unwrap_or(false)
    {
        let _ = raw::identity_create(store, sub, None, 0).await;
    }
}
