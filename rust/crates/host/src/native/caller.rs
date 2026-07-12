//! Project an already-authorized `&Principal` into the minimal, non-replayable wire [`Caller`] the
//! native call frame carries (native-caller-identity scope).
//!
//! One responsibility: the READ that turns the principal the host already gated (`mcp:<tool>:call`
//! fired first, workspace-first) into the `{sub, ws, role, delegated}` a sidecar needs to attribute
//! its per-caller row-filter decision. It mints nothing and copies no bearer material — the frame
//! caller is inert identity, never a token (see [`lb_supervisor::Caller`]).
//!
//! Both native-dispatch entry points project through here so the frame caller is identical whether a
//! call arrives as the direct `native.call` verb (`tool::call_sidecar`) or as a routed
//! `<ext>.<tool>` through the registry adapter (`call::SidecarDispatch`).

use lb_auth::{Principal, Role};
use lb_supervisor::Caller;

/// Project `p` into the wire [`Caller`]. `delegated` is `owner_sub() != sub()` — the same signal
/// `Principal::derive` sets when an actor acts on behalf of a root caller.
pub(super) fn project(p: &Principal) -> Caller {
    Caller {
        sub: p.sub().to_string(),
        ws: p.ws().to_string(),
        role: role_str(p.role()).to_string(),
        delegated: p.owner_sub() != p.sub(),
    }
}

/// The lower-cased wire spelling of a role — matches `#[serde(rename_all = "kebab-case")]` on
/// [`Role`], so the child reads the same token the gateway would serialize.
fn role_str(role: Role) -> &'static str {
    match role {
        Role::SuperAdmin => "super-admin",
        Role::WorkspaceAdmin => "workspace-admin",
        Role::Member => "member",
    }
}
