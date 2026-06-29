//! Seed the two built-in roles (`apikey-read` / `apikey-write`) idempotently (api-keys scope). The
//! scope's resolved open question chose "ensure them idempotently on first key create" — there is no
//! central authz-role seed point (the three token roles are minted into the token, not the role
//! table), so `apikey.create` ensures these here. Re-running is a no-op once the rows exist.

use lb_store::{read, write, Store, StoreError};

use lb_apikey::{apikey_read_caps, apikey_write_caps, ROLE_APIKEY_READ, ROLE_APIKEY_WRITE};
use lb_authz::{Role, ROLE_TABLE};

/// Ensure the `apikey-read` and `apikey-write` role rows exist in workspace `ws`, defining them if
/// absent. Idempotent — a present row is left untouched (so an admin who redefined a same-named
/// custom role is not clobbered). Cheap: one point read per role, one write only when missing.
pub async fn ensure_builtin_roles(store: &Store, ws: &str) -> Result<(), StoreError> {
    ensure_one(store, ws, ROLE_APIKEY_READ, apikey_read_caps()).await?;
    ensure_one(store, ws, ROLE_APIKEY_WRITE, apikey_write_caps()).await?;
    Ok(())
}

/// Define `name` with `caps` iff no role row exists for it yet.
async fn ensure_one(
    store: &Store,
    ws: &str,
    name: &str,
    caps: Vec<String>,
) -> Result<(), StoreError> {
    if read(store, ws, ROLE_TABLE, name).await?.is_some() {
        return Ok(());
    }
    let role = Role::new(name, caps);
    let value = serde_json::to_value(&role).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, ROLE_TABLE, name, &value).await
}
