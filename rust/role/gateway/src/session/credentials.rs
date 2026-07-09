//! The **dev-login** claim builder (login-hardening scope). Maps a `(user, workspace)` login to the
//! claim set the gateway mints into a real signed token.
//!
//! **What changed (the escalation fix).** This file used to hold ONE hardcoded `member_caps()`
//! bundle that *contained admin caps* (`members.add`, `teams.manage`, `roles.define`,
//! `grants.assign`, `user.manage`, `workspace.create`, `dashboard.delete_any`, …), and every login
//! — member or admin — was minted that bundle. So a nominal `Role::Member` was, in practice, a full
//! admin: a live test proved `user:bob`, added only as a plain member, could add members, create
//! teams, and self-grant `mcp:workspace.delete:call` (all `204`). The base bundle here is now the
//! **member-only** set from the durable role catalog (`lb_host::member_role_caps`), and admin caps
//! ride the `workspace-admin` ROLE through `resolve_caps` (the `login` route unions it on top). A
//! member's token no longer carries any admin cap — the route's existing capability check now
//! actually `403`s (deny path exercised for real).
//!
//! **The base floor is the VIEWER set, not the member set (the nav-as-reach fix).** This used to be
//! `member_role_caps()` — so EVERY login, whatever role, was minted the full member floor including
//! the author caps (`rules.*`, `flows.*`, `query.*`, `template.*`, the broad `mcp:*.delete:call`
//! wildcards). That re-widened a `viewer` back to a member: a live session gave `user:bob` a one-page
//! nav, yet he opened the Rules editor by URL because his token carried `mcp:rules.*` regardless of
//! his role. The floor is now the **viewer** bundle — the universal minimum every authenticated
//! principal needs to render a screen they were given (read their dashboards/panels/nav, the
//! `viz.query` render path, their own prefs/layout). Anything above viewer — the AUTHOR delta a
//! `member` holds, the admin caps a `workspace-admin` holds — rides that principal's ROLE grant
//! through `resolve_caps`, which the `login` route unions on top. So a `viewer`-role token stays a
//! viewer (its floor + its viewer role = viewer caps), and a `member`-role token still resolves to
//! full member caps (viewer floor ∪ member role). The nav can finally restrict reach, because reach is
//! gated by caps and a viewer's caps carry no authoring surface.
//!
//! The floor is a working *viewer* session even if the grant-store fold hiccups; `resolve_caps` is
//! the authority for anything above viewer (member author caps, admin, installed-extension tools,
//! custom roles). The credential check that gates whether a token is minted at all lives in
//! [`crate::session::credential`]; this file only shapes the claims once minting is allowed.

use lb_auth::{Claims, Role};

/// The capability strings the base dev claim carries: the built-in **viewer** bundle from the durable
/// role catalog (`lb_host`) — the universal minimum every authenticated principal holds (read + render
/// a screen you were given). One source of truth — the same caps the seeded `role:viewer` record
/// holds. Author caps (a member) and admin caps ride the principal's ROLE via `resolve_caps` (see
/// `routes/login.rs`); they are deliberately NOT in this floor, so a `viewer` is never re-widened.
fn base_viewer_caps() -> Vec<String> {
    lb_host::viewer_role_caps()
}

/// Build the claim set for `user` logging in to `workspace`, valid for `ttl` seconds from `now`.
/// Real signed claims. The base caps are the VIEWER floor; the `login` route unions the principal's
/// resolved role/grant caps (`resolve_caps`) on top, so a member's token carries author caps via the
/// `member` role and an admin's carries admin caps via `workspace-admin` — a viewer's carries neither.
/// The workspace becomes the token's hard wall (§7). The `role` claim stays `Member` (cosmetic — the
/// check path reads `caps`, never `role`, per `lb_auth::Principal`); reach is decided by the caps.
pub fn dev_claims(user: &str, workspace: &str, now: u64, ttl: u64) -> Claims {
    Claims {
        sub: user.to_string(),
        ws: workspace.to_string(),
        role: Role::Member,
        caps: base_viewer_caps(),
        iat: now,
        exp: now.saturating_add(ttl),
        constraint: None,
        run_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The base dev claim carries the LOAD-BEARING VIEWER render-path caps the `mcp:*.<verb>:call`
    /// wildcards miss (`.catalog`/`.pin`) — without any of these the live page silently denies even
    /// though the route-level cap is present (the end-to-end gap these guard against). These are the
    /// caps needed to RENDER a screen you were given; the datasource-REGISTRATION chain
    /// (`datasource.add`/`native.call`/`secret:federation/*:write`) moved to the AUTHOR (member) tier
    /// and is asserted there. See the authoritative split + tests in `lb_host::authz::builtin_roles`.
    #[test]
    fn base_claim_carries_load_bearing_viewer_caps() {
        let caps = base_viewer_caps();
        for needed in [
            "mcp:dashboard.catalog:call",
            "mcp:dashboard.pin:call",
            "mcp:tools.catalog:call",
            "mcp:datasource.list:call",
            "mcp:federation.query:call",
            "mcp:viz.query:call",
            "mcp:nav.resolve:call",
            // reminders nav gate: the frontend `hasCap` checks this concrete string EXACTLY and does
            // not expand the `mcp:*.list:call` wildcard, so the Reminders sidebar entry needs it
            // spelled out like the other Automation surfaces (reminders-nav-missing-list-cap.md).
            "mcp:reminder.list:call",
        ] {
            assert!(
                caps.iter().any(|c| c == needed),
                "base viewer claim must grant {needed}"
            );
        }
    }

    /// The **nav-as-reach** regression, at the claim layer: the base dev floor is a VIEWER — it
    /// carries NONE of the author caps (`rules.*`/`flows.*`/`query.*`/`template.*`/`datasource.add`)
    /// nor the broad write wildcards. This is why a viewer given a one-page nav cannot reach the
    /// Rules/Flows editors by URL: the floor no longer re-widens them to a member. Author caps must
    /// ride `role:member` via `resolve_caps`, never the base floor.
    #[test]
    fn base_claim_carries_no_author_caps() {
        let caps = base_viewer_caps();
        for author_cap in [
            "mcp:rules.save:call",
            "mcp:rules.run:call",
            "mcp:flows.save:call",
            "mcp:query.run:call",
            "mcp:template.save:call",
            "mcp:datasource.add:call",
            "mcp:native.call:call",
            "mcp:ingest.write:call",
            "mcp:store.query:call",
            "mcp:dashboard.save:call",
            "mcp:*.write:call",
            "mcp:*.delete:call",
            "store:*:write",
        ] {
            assert!(
                !caps.iter().any(|c| c == author_cap),
                "base viewer claim must NOT carry author cap {author_cap} (the nav-as-reach leak)"
            );
        }
    }

    /// The escalation regression, at the claim layer: the base dev claim carries NONE of the admin
    /// caps the live `user:bob` abused. Admin caps must ride `role:workspace-admin` via `resolve_caps`,
    /// never the base floor. (`telemetry.purge` stays out too — the CLI parity-deny test relies on it
    /// being ungranted.)
    #[test]
    fn base_claim_carries_no_admin_caps() {
        let caps = base_viewer_caps();
        for admin_cap in [
            "mcp:members.add:call",
            "mcp:teams.manage:call",
            "mcp:roles.define:call",
            "mcp:grants.assign:call",
            "mcp:user.manage:call",
            "mcp:workspace.create:call",
            "mcp:workspace.delete:call",
            "mcp:dashboard.delete_any:call",
            "mcp:prefs.set_default:call",
            "mcp:telemetry.purge:call",
        ] {
            assert!(
                !caps.iter().any(|c| c == admin_cap),
                "base viewer claim must NOT carry admin cap {admin_cap} (the escalation)"
            );
        }
    }
}
