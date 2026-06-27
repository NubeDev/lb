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
        // data-console (Ingest page): the S8 ingest/series verbs, surfaced over the gateway. These
        // are **member-level** — any member may explore + manually write their own series (the
        // producer is the authenticated principal, un-spoofable).
        "mcp:ingest.write:call",
        "mcp:series.read:call",
        "mcp:series.latest:call",
        "mcp:series.find:call",
        "mcp:series.list:call",
        // tag a series entity (member-level): the discovery edges `series.find` intersects. A member
        // may tag their own series; the test gateway's `/_seed/series` route uses this real write path.
        "mcp:tags.add:call",
        // data-console (Data page, the DB browser): the raw-store lens verbs. **ADMIN-ONLY** by
        // decision — they relax the per-record membership gate (gate 3): a raw scan answers "every
        // record in the workspace". The dev principal is a workspace admin (it holds the destructive
        // verbs above), so it carries them; a true member role must NOT. The gateway re-checks each
        // server-side, and a deny-test asserts a token without the cap is refused (data-console risk).
        "mcp:store.tables:call",
        "mcp:store.scan:call",
        "mcp:store.graph:call",
        // coding-workflow scope: the `workflow.*` verbs the approval-gate routes check
        // (`POST /approvals/{id}/request|resolve|start`). The dev member can open an approval,
        // resolve it, and start the gated coding job from the browser; the gateway re-checks each
        // cap server-side (the S6 approval gate itself is enforced regardless of caps). A token
        // WITHOUT these is still refused server-side (workflow_verb_without_the_cap_is_denied).
        "mcp:workflow.request_approval:call",
        "mcp:workflow.resolve_approval:call",
        "mcp:workflow.start_job:call",
        // files/skills scope: the shared-asset surface caps the doc/skill routes check directly
        // (`authorize_doc`/`authorize_skill` gate on `store:doc/{id}` / `store:skill/{id}`, NOT an
        // MCP verb). The dev member may put/get/share/link their docs and manage skills; gate 3
        // (membership/ownership) still decides which *specific* asset they may read. `add_team_member`
        // is gated by `store:doc/*:write` (an admin act at S4), so the dev admin can populate teams.
        "store:doc/*:read",
        "store:doc/*:write",
        "store:skill/*:read",
        "store:skill/*:write",
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
