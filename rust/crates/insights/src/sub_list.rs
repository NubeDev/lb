//! `sub_list` — list subscriptions (insight-subscriptions-scope.md).
//!
//! Member-default: the caller's OWN subs. Admin lens: all ws subs (the host threads an `all`
//! flag, gated on the admin cap). The list shape is the same either way.
//!
//! **STUB**: body deferred — see the punch-list.

use lb_store::Store;

use crate::error::InsightsError;
use crate::subscription::Subscription;

/// List subs in workspace `ws`. If `all` is true, returns every ws sub (admin lens); else only
/// `owner`'s subs. The host gates `all` on the admin cap.
// SCOPE: docs/scope/insights/insight-subscriptions-scope.md §"Verb surface"
pub async fn sub_list(
    _store: &Store,
    _ws: &str,
    _owner: &str,
    _all: bool,
) -> Result<Vec<Subscription>, InsightsError> {
    // 1. If `all` ⇒ scan the whole `insight_sub` table in ws (admin lens; the host checked cap).
    // 2. Else ⇒ `list(store, ws, TABLE, "owner", owner)`.
    // 3. Decode each row; return.
    todo!("insights: sub list (own + admin lens) — SCOPE: subscriptions-scope.md §Verb surface")
}
