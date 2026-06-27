//! The **dev-login** credential map — the ONE non-real piece of the session (collaboration scope,
//! Non-goals: "no IdP yet; the token path is real even if the credential check starts as a dev-login").
//!
//! It maps a `(user, workspace)` login request to the claim set the gateway then mints into a real
//! signed token. There is no password DB here — a real credential check / IdP plugs in *here*, behind
//! the same `mint`/`verify` seam, without touching any route. The granted caps are the full member
//! set for the collaboration surfaces (channels, members, inbox, outbox, workspace directory) so the
//! demo principal can exercise every wired verb; a narrower dev principal is built by the tests to
//! prove the deny path.

use lb_auth::{Claims, Role};

/// The capability strings a dev member is granted — every collaboration verb's gate. Channel pub/sub
/// over `*` (post/read/list/create any channel) plus the MCP verb caps the new services check.
fn member_caps() -> Vec<String> {
    [
        "bus:chan/*:pub",
        "bus:chan/*:sub",
        "mcp:members.list:call",
        "mcp:members.add:call",
        "mcp:inbox.list:call",
        "mcp:inbox.resolve:call",
        "mcp:outbox.status:call",
        "mcp:workspace.list:call",
        "mcp:workspace.create:call",
        // admin-crud: the dev principal is a workspace admin so the console can exercise every
        // destructive verb. The gateway re-checks each on the server — the UI cap-gate is only a
        // convenience (admin-console scope). `workspace.purge` is the higher hard-delete ceiling.
        "mcp:workspace.delete:call",
        "mcp:workspace.purge:call",
        "mcp:user.manage:call",
        "mcp:user.disable:call",
        "mcp:teams.manage:call",
        "mcp:teams.list:call",
        "mcp:grants.assign:call",
        "mcp:grants.list:call",
        "mcp:roles.define:call",
        "mcp:roles.list:call",
        // admin-console slice 4: the extensions console lifecycle verbs, so the dev admin can list +
        // enable/disable/uninstall extensions from the browser. The gateway re-checks each on the
        // server; the UI cap-gate (showing the Extensions section) is convenience.
        "mcp:ext.list:call",
        "mcp:ext.disable:call",
        "mcp:ext.uninstall:call",
        // admin-console: publish (upload) a signed extension artifact over POST /extensions. The host
        // verb verify-before-stores; the gateway re-checks this cap server-side.
        "mcp:ext.publish:call",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Build the claim set for `user` logging in to `workspace`, valid for `ttl` seconds from `now`.
/// Real signed claims — only the *credential check* (here, "any user, any workspace") is the
/// dev-login stand-in. The workspace becomes the token's hard wall (§7).
pub fn dev_claims(user: &str, workspace: &str, now: u64, ttl: u64) -> Claims {
    Claims {
        sub: user.to_string(),
        ws: workspace.to_string(),
        role: Role::Member,
        caps: member_caps(),
        iat: now,
        exp: now.saturating_add(ttl),
    }
}
