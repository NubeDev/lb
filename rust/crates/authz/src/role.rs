//! **Roles**: `role(name -> caps[])` records — named cap bundles a workspace can define beyond the
//! three built-ins (authz-grants scope). `define` / `list` / `caps`, raw and workspace-namespaced.
//!
//! Assigning a role to a user/team is *not* a verb here — it is an ordinary `grant_assign` of the
//! synthetic cap `role:<name>` to that subject; [`resolve_caps`](crate::resolve_caps) expands it.
//! Keeping role-assignment as a grant means one assign/revoke path, not two. `define` is idempotent
//! on the role name (re-defining replaces the cap set — last write wins).
//!
//! Built-in roles (super-admin / workspace-admin / member) are seeded by the caller, not here; a
//! custom role may only bundle caps the definer themselves holds (the no-widening rule lives in the
//! host service, which has the principal — this crate is the raw store).

use lb_store::{list as store_list, read, write, Store, StoreError};
use serde::{Deserialize, Serialize};

/// The store table roles live in, within a workspace namespace.
pub const ROLE_TABLE: &str = "role";

/// The constant `kind` discriminant so [`role_list`] can equality-filter every role row.
const KIND: &str = "role";

/// A role: a named bundle of capability strings, workspace-scoped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    /// The role name (`operator`, `auditor`).
    pub name: String,
    /// The capability strings this role bundles.
    pub caps: Vec<String>,
    /// Constant discriminant so `role_list` selects every row.
    pub kind: String,
}

impl Role {
    pub fn new(name: impl Into<String>, caps: Vec<String>) -> Self {
        Self {
            name: name.into(),
            caps,
            kind: KIND.to_string(),
        }
    }
}

/// Define (or replace) role `name` with `caps` in workspace `ws`. Idempotent on the name.
pub async fn role_define(
    store: &Store,
    ws: &str,
    name: &str,
    caps: &[String],
) -> Result<(), StoreError> {
    let role = Role::new(name, caps.to_vec());
    let value = serde_json::to_value(&role).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, ROLE_TABLE, name, &value).await
}

/// The caps role `name` bundles in `ws`, or empty if the role is undefined (an undefined role
/// contributes nothing — deny by default).
pub async fn role_caps(store: &Store, ws: &str, name: &str) -> Result<Vec<String>, StoreError> {
    match read(store, ws, ROLE_TABLE, name).await? {
        Some(v) => {
            let role: Role =
                serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(role.caps)
        }
        None => Ok(Vec::new()),
    }
}

/// Every role defined in workspace `ws` (for the roles/grants admin read surface).
pub async fn role_list(store: &Store, ws: &str) -> Result<Vec<Role>, StoreError> {
    let rows = store_list(store, ws, ROLE_TABLE, "kind", KIND).await?;
    rows.into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string())))
        .collect()
}
