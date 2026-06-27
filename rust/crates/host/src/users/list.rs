//! `user.list` — enumerate the workspace's users for the admin UI (admin-crud scope). Gated by
//! `mcp:user.manage:call`, workspace-first. Returns the **credential-free** [`UserView`] — the
//! `cred_ref` field is never serialized out, so `user.list` can never leak the credential.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::{list as store_list, Store};

use super::error::UsersError;
use super::model::{UserRecord, UserView, KIND, TABLE};

/// Return every user in workspace `ws` as credential-free views, for `principal`. Sorted by id for
/// a stable table (testing §3 — deterministic order).
pub async fn user_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<UserView>, UsersError> {
    authorize_tool(principal, ws, "user.manage").map_err(|_| UsersError::Denied)?;
    let rows = store_list(store, ws, TABLE, "kind", KIND).await?;
    let mut views: Vec<UserView> = rows
        .into_iter()
        .map(|v| {
            serde_json::from_value::<UserRecord>(v)
                .map(UserView::from)
                .map_err(|e| lb_store::StoreError::Decode(e.to_string()).into())
        })
        .collect::<Result<_, UsersError>>()?;
    views.sort_by(|a, b| a.user.cmp(&b.user));
    Ok(views)
}
