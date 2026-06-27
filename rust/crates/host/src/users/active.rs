//! `user.disable` / `user.enable` — flip the `active` flag the login path checks (admin-crud scope).
//! Gated by `mcp:user.disable:call`, workspace-first. `disable` is the safest true lockout: it kills
//! **minting** (the login path refuses), so a disabled user cannot re-mint to refresh stale caps —
//! the mitigation the freshness asymmetry calls for (a removed member keeps inherited caps until
//! re-mint, but a disabled user can't re-mint at all).
//!
//! Idempotent: disabling an already-disabled user (or an absent one) is a success — flipping an
//! absent record is a no-op write of the desired state, never a cross-workspace reach.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::{read, write, Store};

use super::error::UsersError;
use super::model::{UserRecord, TABLE};

/// Disable `user` in `ws`: `active=false`. The login path then refuses to mint a session for them.
pub async fn user_disable(
    store: &Store,
    principal: &Principal,
    ws: &str,
    user: &str,
) -> Result<(), UsersError> {
    set_active(store, principal, ws, user, false).await
}

/// Enable `user` in `ws`: `active=true`. Restores the ability to mint a session.
pub async fn user_enable(
    store: &Store,
    principal: &Principal,
    ws: &str,
    user: &str,
) -> Result<(), UsersError> {
    set_active(store, principal, ws, user, true).await
}

/// Flip `active` for `user` in `ws`. Gated by `mcp:user.disable:call`. If the record is absent the
/// flip is a no-op success (idempotent; never errors, never crosses the wall).
async fn set_active(
    store: &Store,
    principal: &Principal,
    ws: &str,
    user: &str,
    active: bool,
) -> Result<(), UsersError> {
    authorize_tool(principal, ws, "user.disable").map_err(|_| UsersError::Denied)?;
    let Some(value) = read(store, ws, TABLE, user).await? else {
        return Ok(()); // absent → nothing to flip; idempotent success.
    };
    let mut record: UserRecord =
        serde_json::from_value(value).map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
    record.active = active;
    let value =
        serde_json::to_value(&record).map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, user, &value).await?;
    Ok(())
}
