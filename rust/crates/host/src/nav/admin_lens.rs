//! `is_workspace_admin` — is the caller a workspace admin, for the nav no-lockout rule
//! ([`scope/nav/nav-no-lockout-scope.md`])?
//!
//! **Why this exists.** A curated nav is meant to SHAPE a member's menu, not REPLACE an administrator's
//! console. The 4-tier resolve (`resolve.rs::pick_nav`) auto-applies tier-2 (a team-shared nav) and
//! tier-3 (the workspace default) to *anyone*, so an admin who merely belongs to a team someone shared a
//! 1-page nav to — or a workspace with any default — gets their whole admin console silently subtracted
//! from the rail, with no in-app way back. That is a lockout, not a lens. The fix: an admin is only ever
//! narrowed by their OWN explicit pick (tier 1); tiers 2/3 are skipped for them.
//!
//! **The admin signal is cap-based, one definition.** We mirror the shell's `isAdmin` exactly (any one
//! of the admin-section caps present) rather than invent a nav-local notion of admin — so "who is an
//! admin" has a single source of truth. The token's caps are the authority (resolved identically on
//! every node — rule 1). Keep this list in lockstep with `ui/src/lib/session/admin-caps.ts`
//! `ADMIN_SECTION_CAPS`.

use lb_auth::Principal;

use crate::authz::holds_cap;

/// The admin-section marker caps — presence of ANY one means the caller sees the admin console, so a
/// curated nav must not silently replace it. Mirrors the UI's `ADMIN_SECTION_CAPS`. These are FULL
/// `surface:resource:action` grant strings — [`holds_cap`] parses the grant grammar, not a bare tool
/// name.
const ADMIN_MARKER_CAPS: &[&str] = &[
    "mcp:user.manage:call",
    "mcp:teams.manage:call",
    "mcp:grants.assign:call",
    "mcp:workspace.delete:call",
    "mcp:ext.list:call",
    "mcp:devkit.templates:call",
    "mcp:apikey.manage:call",
    "mcp:webhook.manage:call",
    "mcp:members.manage:call",
];

/// Is `principal` a workspace admin in `ws`? True iff they hold any admin-section cap. Used by
/// `pick_nav` to skip the auto-apply tiers (team share / workspace default) for admins — an admin is
/// narrowed only by their own explicit personal pick.
pub fn is_workspace_admin(principal: &Principal, ws: &str) -> bool {
    ADMIN_MARKER_CAPS
        .iter()
        .any(|cap| holds_cap(principal, ws, cap))
}
