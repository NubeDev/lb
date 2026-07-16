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
//!
//! **Why an EXACT match, not [`holds_cap`].** This is a *classification* probe ("is this bundle an
//! admin bundle?"), not an authorization check ("may this caller do X?"). `holds_cap` answers the
//! second question and is wildcard-aware by design — it is the right matcher for a GATE and the wrong
//! one for a LABEL. Probing it with `mcp:ext.list:call` returned true for the member bundle's broad
//! `mcp:*.list:call`, so every member and viewer read as an admin, `pick_nav` skipped tiers 2/3 for
//! them, and a curated nav NEVER applied — the feature was inert in production (observed live
//! 2026-07-16: `user:bob`, `role:member`, resolved `fallback` against a team-shared nav he could read).
//!
//! Those wildcards are now GONE from the member/viewer bundles (`authz::builtin_roles` — they were
//! also authorizing ten admin-only caps for real, which was the graver half of the same bug), so an
//! exact match and a `holds_cap` match would agree here today. The exact match stays regardless: the
//! agreement is a property of the current bundles, not of the question being asked, and this call site
//! should not silently depend on a bundle never growing a wildcard again. Classification asks what a
//! caller IS; a wildcard describes what they MAY DO. `caps_hold_admin` (the authoritative admin signal
//! for the native-caller frame) already matched exactly — this module was the deviation, and the
//! module's own claim of ONE definition of admin is now true.

use lb_auth::Principal;

/// The admin-section marker caps — presence of ANY one means the caller sees the admin console, so a
/// curated nav must not silently replace it. Mirrors the UI's `ADMIN_SECTION_CAPS`. These are FULL
/// `surface:resource:action` grant strings, matched EXACTLY (see the module note).
///
/// Every entry must be admin-ONLY (`ADMIN_ONLY_CAPS` in `authz::builtin_roles`) — a cap the member
/// bundle also grants would classify every member as an admin. `member_bundle_does_not_read_as_admin`
/// pins that. (`mcp:devkit.templates:call` was listed here but lives in the member `AUTHOR_CAPS`
/// bundle — the authoring toolchain, not the admin console — so it is not a marker.)
const ADMIN_MARKER_CAPS: &[&str] = &[
    "mcp:user.manage:call",
    "mcp:teams.manage:call",
    "mcp:grants.assign:call",
    "mcp:workspace.delete:call",
    "mcp:ext.list:call",
    "mcp:apikey.manage:call",
    "mcp:webhook.manage:call",
    "mcp:members.manage:call",
];

/// Is `principal` a workspace admin in `ws`? True iff their caps LITERALLY contain an admin-section
/// marker cap. Used by `pick_nav` to skip the auto-apply tiers (team share / workspace default) for
/// admins — an admin is narrowed only by their own explicit personal pick.
///
/// `ws` is part of the signature because the admin signal is per-workspace by definition; the caps on
/// a `Principal` are already resolved for the workspace it was minted against.
pub fn is_workspace_admin(principal: &Principal, _ws: &str) -> bool {
    principal
        .caps()
        .iter()
        .any(|cap| ADMIN_MARKER_CAPS.contains(&cap.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authz::{
        admin_only_caps, holds_cap, member_role_caps, viewer_role_caps, workspace_admin_role_caps,
    };

    const WS: &str = "acme";

    /// **Every marker must be admin-ONLY** — asserted against `ADMIN_ONLY_CAPS`, not trusted. This is
    /// the property that makes the list self-checking: a marker that is not admin-only misclassifies
    /// whoever else holds it, which is exactly how `mcp:devkit.templates:call` (the member authoring
    /// toolchain, in `AUTHOR_CAPS`) sat in this list and helped label every member an admin.
    ///
    /// The two directions below are stricter than "is it in the list": a marker must be admin-only by
    /// TIER (present in `ADMIN_ONLY_CAPS`) and unreachable in practice (no member/viewer bundle may
    /// AUTHORIZE it — the wildcard-span check, mirroring `builtin_roles`'s own invariant). Either one
    /// failing means the marker labels non-admins.
    #[test]
    fn every_marker_cap_is_admin_only() {
        let admin_only = admin_only_caps();
        for marker in ADMIN_MARKER_CAPS {
            assert!(
                admin_only.iter().any(|c| c == marker),
                "marker {marker} is not in ADMIN_ONLY_CAPS — a cap a member/viewer can also hold \
                 must never mark the admin console (this is how mcp:devkit.templates:call, an \
                 AUTHOR cap, ended up labelling every member an admin)"
            );
        }
        for (role, caps) in [
            ("member", member_role_caps()),
            ("viewer", viewer_role_caps()),
        ] {
            let p = Principal::routed("user:probe", WS, caps);
            for marker in ADMIN_MARKER_CAPS {
                assert!(
                    !holds_cap(&p, WS, marker),
                    "the {role} bundle AUTHORIZES the admin marker {marker} — even though this \
                     module matches exactly, a bundle that can reach a marker means the marker is \
                     not an admin-only signal"
                );
            }
        }
    }

    /// The no-lockout rule only holds if the admin signal is EXACT. A plain member must never read as
    /// an admin: `pick_nav` skips tiers 2/3 for admins, so a member misread as admin silently loses
    /// their team-shared nav and workspace default and falls back to the built-in rail — the curated
    /// menu never applies. Observed live (2026-07-16): `user:bob`, holding only `role:member`,
    /// resolved `{"source":"fallback"}` against a team-shared nav he could `nav.get` (200).
    #[test]
    fn member_bundle_does_not_read_as_admin() {
        let caps = member_role_caps();
        let offenders: Vec<&str> = ADMIN_MARKER_CAPS
            .iter()
            .copied()
            .filter(|cap| caps.iter().any(|c| c == cap))
            .collect();
        assert!(
            offenders.is_empty(),
            "no admin marker cap may live in the member bundle — offenders: {offenders:?}"
        );
        let bob = Principal::routed("user:bob", WS, caps);
        assert!(
            !is_workspace_admin(&bob, WS),
            "a plain member must NOT read as workspace admin (the wildcard collision: the member \
             bundle's mcp:*.list:call / mcp:*.delete:call must not satisfy an admin marker)"
        );
    }

    /// A viewer is even narrower — same wall.
    #[test]
    fn viewer_bundle_does_not_read_as_admin() {
        let ben = Principal::routed("user:ben", WS, viewer_role_caps());
        assert!(!is_workspace_admin(&ben, WS));
    }

    /// The other direction: a real admin MUST read as admin, or the no-lockout rule stops protecting
    /// the console it exists for.
    #[test]
    fn workspace_admin_bundle_reads_as_admin() {
        let ada = Principal::routed("user:ada", WS, workspace_admin_role_caps());
        assert!(is_workspace_admin(&ada, WS));
    }
}
