//! The identity view types the gateway returns. `IdentityView` is the directory record minus nothing
//! (identity is already secret-free — no credential, decision #7); the named view keeps the wire shape
//! stable if the raw `Identity` grows internal fields later. `IdentityWorkspace` is one row of the
//! `identity.workspaces` resolution.

use serde::{Deserialize, Serialize};

/// A global identity as the admin UI / resolver sees it — secret-free.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityView {
    pub sub: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub created_ts: u64,
}

impl From<lb_authz::Identity> for IdentityView {
    fn from(i: lb_authz::Identity) -> Self {
        Self {
            sub: i.sub,
            display_name: i.display_name,
            created_ts: i.created_ts,
        }
    }
}

/// One workspace an identity is a member of — the `identity.workspaces` resolution row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityWorkspace {
    pub ws: String,
    pub name: String,
}
