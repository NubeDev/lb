//! `user_login_check` — the **login-path seam**: may `user` mint a session in `ws`? (admin-crud
//! scope — "disable bites the login path"). Called by the gateway `login` route *before* it mints a
//! token, so a disabled or deleted user is refused at the source.
//!
//! This is **not** capability-gated — there is no principal yet (we are deciding whether to create
//! one). It is workspace-scoped: it reads `ws`'s own namespace, so it can only ever consult `ws`'s
//! user record (the hard wall holds even pre-mint).
//!
//! Policy (the resolved decision): if **no** user record exists, minting is **allowed** — the
//! dev-login auto-seeds principals on first login (collaboration's behavior preserved; admin records
//! are opt-in). If a record exists, it must be **present (not tombstoned) and `active`**. So
//! `user.disable` / `user.delete` immediately stop minting; an un-administered workspace still works.

use lb_store::{read, Store};

use super::error::UsersError;
use super::model::{UserRecord, TABLE, TOMBSTONE};

/// May `user` mint a session in `ws`? `Ok(())` if allowed; [`UsersError::Disabled`] if a record
/// exists and is disabled or deleted. Absent record → allowed (auto-seed on first login).
pub async fn user_login_check(store: &Store, ws: &str, user: &str) -> Result<(), UsersError> {
    let Some(value) = read(store, ws, TABLE, user).await? else {
        return Ok(()); // no record yet — dev-login auto-seeds; minting allowed.
    };
    // A tombstoned (deleted) record reads as a non-`user` kind — refuse minting.
    if value.get("kind").and_then(|k| k.as_str()) == Some(TOMBSTONE) {
        return Err(UsersError::Disabled);
    }
    let record: UserRecord =
        serde_json::from_value(value).map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
    if record.active {
        Ok(())
    } else {
        Err(UsersError::Disabled)
    }
}
