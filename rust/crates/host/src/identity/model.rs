//! The identity view types the gateway returns. `IdentityView` is the directory record minus nothing
//! (identity is already secret-free — no credential, decision #7); the named view keeps the wire shape
//! stable if the raw `Identity` grows internal fields later. `IdentityWorkspace` is one row of the
//! `identity.workspaces` resolution.

use serde::{Deserialize, Serialize};

/// A global identity as the admin UI / resolver sees it — secret-free. Carries the `email` login
/// handle (email-login scope) for the admin console; the credential (hash) is NEVER in this view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityView {
    pub sub: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// The globally-unique email login handle (lower-cased), if set. Non-secret — safe to list to an
    /// admin. `None` for a machine/agent identity or one provisioned before an email was set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub created_ts: u64,
}

impl From<lb_authz::Identity> for IdentityView {
    fn from(i: lb_authz::Identity) -> Self {
        Self {
            sub: i.sub,
            display_name: i.display_name,
            email: i.email,
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
