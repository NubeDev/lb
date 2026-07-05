//! `sub_list` — list subscriptions (insight-subscriptions-scope.md).
//!
//! Member-default: the caller's OWN subs. Admin lens: all ws subs (the host threads an `all`
//! flag, gated on the admin cap). The list shape is the same either way.
//!
//! **STUB**: body deferred — see the punch-list.

use lb_store::Store;

use crate::error::InsightsError;
use crate::subscription::{Subscription, TABLE};
use crate::table_scan::scan_all;

/// List subs in workspace `ws`. If `all` is true, returns every ws sub (admin lens); else only
/// `owner`'s subs. The host gates `all` on the admin cap.
// SCOPE: docs/scope/insights/insight-subscriptions-scope.md §"Verb surface"
pub async fn sub_list(
    store: &Store,
    ws: &str,
    owner: &str,
    all: bool,
) -> Result<Vec<Subscription>, InsightsError> {
    let rows = if all {
        // Admin lens — every sub in the workspace (the host verified the admin cap).
        scan_all(store, ws, TABLE).await?
    } else {
        // Own lens — filter by owner via the cheap `data.owner` field path.
        lb_store::list(store, ws, TABLE, "owner", owner).await?
    };
    let subs = rows
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();
    Ok(subs)
}
