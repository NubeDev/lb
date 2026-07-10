//! The host's [`BuiltinRoleCaps`] — the live cap-bundle matcher injected into the resolver so a new
//! built-in cap reaches already-seeded workspaces without a re-seed (builtin-role-freshness scope).
//!
//! `lb-authz`'s resolver reads the STORED role record for a granted role. Built-in role rows are
//! seeded idempotently ([`ensure_builtin_authz_roles`] writes a row only when absent), so a workspace
//! seeded before a new built-in cap was added keeps the stale row forever — and the resolver, reading
//! that row, never grants the new cap. The viewer tier dodged this because the login floor
//! (`credentials.rs`) calls the LIVE `viewer_role_caps()`; author/admin caps ride the stored record,
//! so they went stale (the frozen-role footgun).
//!
//! The durable fix: the resolver UNIONS the live bundle on top of the stored record for a granted
//! built-in role. `lb-authz` is pure (no host dep), so the live bundles are injected via the
//! [`BuiltinRoleCaps`] trait. [`LiveBuiltinRoleCaps`] is the host impl — the single place that maps a
//! built-in name to its authoritative `*_role_caps()`. Every host caller of the resolver
//! (login mint, apikey auth/get, reminder fire, dashboard access_check, the access console) passes it,
//! so the fix is universal and the stored row is no longer load-bearing for built-in names.
//!
//! Why union (not replace): a `grant_assign(Subject::Role("member"), cap)` — how an installed
//! extension's tools reach every member — still honours the stored record. Union keeps that while
//! guaranteeing the live built-in set is a floor. See
//! `docs/debugging/authz/builtin-role-row-frozen-stale-on-new-caps.md`.

use lb_authz::BuiltinRoleCaps;

use crate::authz::builtin_roles::{
    member_role_caps, viewer_role_caps, workspace_admin_role_caps, ROLE_MEMBER, ROLE_VIEWER,
    ROLE_WORKSPACE_ADMIN,
};

/// The host's live built-in role cap source — maps the three built-in names to their authoritative
/// `*_role_caps()` bundles. Pass `&LiveBuiltinRoleCaps` to `resolve_caps_with` /
/// `resolve_subject_caps_with` / the `_sourced_with` twins.
pub struct LiveBuiltinRoleCaps;

impl BuiltinRoleCaps for LiveBuiltinRoleCaps {
    fn live_caps(&self, name: &str) -> Option<Vec<String>> {
        match name {
            ROLE_VIEWER => Some(viewer_role_caps()),
            ROLE_MEMBER => Some(member_role_caps()),
            ROLE_WORKSPACE_ADMIN => Some(workspace_admin_role_caps()),
            // A custom role (or a typo) has no live bundle — the resolver reads only its stored record.
            _ => None,
        }
    }
}
