//! `user.create` — seed a durable user record over the dev store (admin-crud scope). Gated by
//! `mcp:user.manage:call`, workspace-first. Idempotent on `(ws, user)` — re-creating upserts the
//! role / cred_ref / ts (last write wins), starting `active=true`.
//!
//! This seeds a **dev** credential reference; the record is the genuinely-missing primitive that
//! makes identity administrable (you can now list/disable/delete a user). The real IdP attaches at
//! `cred_ref` later behind the same seam — no password DB here (Non-goals).

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::{write, Store};

use super::error::UsersError;
use super::model::{UserRecord, TABLE};

/// Create (or update) user `user` in workspace `ws` as `principal`. `role` is a UI hint;
/// `cred_ref` references the mediated dev credential. Returns the credential-free view is the
/// caller's job — this returns unit (the record is written, never echoed with its cred_ref).
pub async fn user_create(
    store: &Store,
    principal: &Principal,
    ws: &str,
    user: &str,
    role: &str,
    cred_ref: &str,
    ts: u64,
) -> Result<(), UsersError> {
    authorize_tool(principal, ws, "user.manage").map_err(|_| UsersError::Denied)?;
    let record = UserRecord::new(user, role, cred_ref, ts);
    let value =
        serde_json::to_value(&record).map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, user, &value).await?;
    Ok(())
}
