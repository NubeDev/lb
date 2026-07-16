//! builtin-role-freshness scope (mandatory regression): the frozen-built-in-role-row footgun.
//!
//! A built-in role row is seeded idempotently (written only when absent), so a workspace seeded
//! BEFORE a new built-in cap was added keeps the stale row forever. Pre-fix, `resolve_caps` read
//! only that stored record, so the new cap never reached the subject's token until someone manually
//! deleted + re-seeded the role row (the throwaway `reseed_roles.rs` the reports demo needed).
//!
//! The fix is the `BuiltinRoleCaps` callback: for a granted BUILT-IN role, `resolve_caps_with` takes
//! the live bundle as authoritative and does not consult the stored record at all, so a built-in cap
//! change takes effect the moment code ships. This test pins BOTH halves:
//!   - `resolve_caps` (no builtins) reads the stale row → the new cap is MISSING (the pre-fix bug).
//!   - `resolve_caps_with` (+ live builtins) → the live bundle is what resolves (the fix).
//!
//! **This was a UNION until 2026-07-16 and is now a REPLACE.** A union made the live bundle a *floor*:
//! it could add a cap to a stale workspace but never remove one — fine while bundles only grow, a
//! security hole the moment one shrinks. When the broad `mcp:*.list:call` / `mcp:*.delete:call`
//! wildcards were removed from the member bundle (they authorized ten admin-only caps —
//! `debugging/auth/member-wildcard-satisfies-admin-cap.md`), every workspace seeded by the older binary
//! kept them: the seed is create-only, so the stale row survives the upgrade and the union folded its
//! wildcards straight back into every member's token. The removal was inert on exactly the deployments
//! that had the bug. Replace keeps what the union was FOR — a direct
//! `grant_assign(Subject::Role("member"), cap)` resolves through the role-subject recursion, not the
//! record (pinned by `live_builtin_caps_keep_direct_role_subject_grants`).
//!
//! Real store, seeded via the real write path (`role_define` + `grant_assign`); no mocks.

use lb_authz::{
    grant_assign, resolve_caps, resolve_caps_with, role_define, BuiltinRoleCaps, NoBuiltinRoleCaps,
    Subject,
};
use lb_store::{Store, StoreError};

const WS: &str = "acme";

/// A live built-in cap source that knows `member` carries `mcp:report.save:call` (the cap the
/// reports feature added — the one the stale row in the dev store was missing). This stands in for
/// the host's `LiveBuiltinRoleCaps` (which maps the three names to `*_role_caps()`); the pure
/// `lb-authz` crate can't see the host, so the test injects the same shape.
struct LiveMemberCaps {
    member: Vec<String>,
}

impl BuiltinRoleCaps for LiveMemberCaps {
    fn live_caps(&self, name: &str) -> Option<Vec<String>> {
        match name {
            "member" => Some(self.member.clone()),
            _ => None,
        }
    }
}

/// The regression: a STALE stored `member` row (seeded BEFORE `mcp:report.save:call` existed) +
/// `resolve_caps_with` (+ live builtins) → the new cap IS resolved. And `resolve_caps` (no builtins)
/// → it is NOT (proving the live bundle is what closes the gap, not the stored row).
#[tokio::test]
async fn live_builtin_caps_close_the_frozen_role_row() -> Result<(), StoreError> {
    let store = Store::memory().await?;

    // Seed the `member` role with a STALE cap set — what an already-seeded workspace holds: the OLD
    // bundle, BEFORE `mcp:report.save:call` was added to the built-in author caps.
    role_define(&store, WS, "member", &["mcp:dashboard.save:call".into()]).await?;
    // bob is a member.
    let bob = Subject::User("bob".into());
    grant_assign(&store, WS, &bob, "role:member").await?;

    // Pre-fix fold (no live built-ins): reads ONLY the stale stored row → report.save is MISSING.
    let stale = resolve_caps(&store, WS, "bob").await?;
    assert!(
        !stale.contains(&"mcp:report.save:call".to_string()),
        "pre-fix: the stale stored member row must NOT carry the new cap (the bug)"
    );
    assert!(
        stale.contains(&"mcp:dashboard.save:call".to_string()),
        "the stored row's existing cap is still read"
    );

    // Fixed fold (+ live built-ins): the live member bundle is authoritative → report.save IS
    // resolved, even though the stored row never had it.
    let live = LiveMemberCaps {
        member: vec![
            "mcp:dashboard.save:call".into(),
            "mcp:report.save:call".into(),
        ],
    };
    let fixed = resolve_caps_with(&store, WS, "bob", &live).await?;
    assert!(
        fixed.contains(&"mcp:report.save:call".to_string()),
        "post-fix: the live member bundle must reach bob's caps without a re-seed (the durable fix)"
    );
    assert!(
        fixed.contains(&"mcp:dashboard.save:call".to_string()),
        "a cap in BOTH the row and the live bundle is still resolved"
    );

    Ok(())
}

/// **The other direction — a cap REMOVED from a built-in bundle must disappear from a stale
/// workspace.** This is the half a union could never do, and the reason it became a replace
/// (2026-07-16).
///
/// The stored `member` row here is what an older binary seeded: it carries `mcp:*.list:call`, the
/// broad wildcard that authorized `teams.list` / `roles.list` / `grants.list` / `invite.list` and
/// eight more admin-only caps. The live bundle no longer has it. Under the old UNION the wildcard
/// folded back in from the row and every member kept the leak forever — the seed is create-only, so
/// no upgrade path ever rewrote the row. Under REPLACE the live bundle is the whole answer for a
/// built-in name and the cap is gone the moment the fixed binary boots, with no re-seed and no
/// migration.
#[tokio::test]
async fn live_builtin_caps_replace_the_stale_role_row() -> Result<(), StoreError> {
    let store = Store::memory().await?;

    // A workspace seeded by the OLD binary: the member row carries the leaky wildcard.
    role_define(
        &store,
        WS,
        "member",
        &["mcp:dashboard.save:call".into(), "mcp:*.list:call".into()],
    )
    .await?;
    let bob = Subject::User("bob".into());
    grant_assign(&store, WS, &bob, "role:member").await?;

    // The stored row still has it — nothing rewrote it (create-only seed).
    let stale = resolve_caps(&store, WS, "bob").await?;
    assert!(
        stale.contains(&"mcp:*.list:call".to_string()),
        "precondition: the stale stored row carries the wildcard the upgrade is meant to remove"
    );

    // The upgraded binary's live bundle: the wildcard is gone.
    let live = LiveMemberCaps {
        member: vec!["mcp:dashboard.save:call".into()],
    };
    let fixed = resolve_caps_with(&store, WS, "bob", &live).await?;
    assert!(
        !fixed.contains(&"mcp:*.list:call".to_string()),
        "a cap REMOVED from the live built-in bundle must NOT resolve from the stale stored row — \
         under the old union it did, so the wildcard fix was inert on every existing deployment"
    );
    assert!(
        fixed.contains(&"mcp:dashboard.save:call".to_string()),
        "the live bundle's own caps still resolve"
    );

    Ok(())
}

/// `NoBuiltinRoleCaps` is the explicit zero builtins impl — it MUST equal the no-arg `resolve_caps`
/// (the zero-arg entry point is defined as `resolve_caps_with(.., &NoBuiltinRoleCaps)`). Pins the
/// equivalence so the zero-arg path never silently drifts from "no builtins".
#[tokio::test]
async fn no_builtin_role_caps_equals_raw_resolve_caps() -> Result<(), StoreError> {
    let store = Store::memory().await?;
    role_define(&store, WS, "member", &["mcp:dashboard.save:call".into()]).await?;
    let bob = Subject::User("bob".into());
    grant_assign(&store, WS, &bob, "role:member").await?;

    let raw = resolve_caps(&store, WS, "bob").await?;
    let with_none = resolve_caps_with(&store, WS, "bob", &NoBuiltinRoleCaps).await?;
    assert_eq!(raw, with_none, "NoBuiltinRoleCaps must match the raw fold");
    Ok(())
}

/// A custom role has NO live bundle → `resolve_caps_with` reads only its stored record, exactly like
/// `resolve_caps`. The union must not invent caps for roles the builtins map doesn't know.
#[tokio::test]
async fn custom_role_unaffected_by_builtin_union() -> Result<(), StoreError> {
    let store = Store::memory().await?;
    role_define(&store, WS, "auditor", &["store:audit/log:read".into()]).await?;
    let bob = Subject::User("bob".into());
    grant_assign(&store, WS, &bob, "role:auditor").await?;

    let live = LiveMemberCaps {
        member: vec!["mcp:report.save:call".into()],
    };
    let caps = resolve_caps_with(&store, WS, "bob", &live).await?;
    assert_eq!(
        caps,
        vec!["store:audit/log:read".to_string()],
        "a custom role resolves from its stored record only — the built-in union does not touch it"
    );
    Ok(())
}

/// **What the replace must NOT break.** A direct grant on the built-in role SUBJECT — how an
/// installed extension's page tools reach every member without editing the built-in record — is still
/// honoured. This was the stated reason the live caps were unioned rather than replaced; it survives
/// the replace because that grant resolves through the role-subject RECURSION, not through the role's
/// stored record. The two were conflated; this test is the proof they are separable.
#[tokio::test]
async fn live_builtin_caps_keep_direct_role_subject_grants() -> Result<(), StoreError> {
    let store = Store::memory().await?;
    role_define(&store, WS, "member", &["mcp:dashboard.save:call".into()]).await?;
    let bob = Subject::User("bob".into());
    grant_assign(&store, WS, &bob, "role:member").await?;
    // An extension grants an extra cap to the role subject directly (the install path).
    grant_assign(
        &store,
        WS,
        &Subject::Role("member".into()),
        "mcp:ext.custom_tool:call",
    )
    .await?;

    let live = LiveMemberCaps {
        member: vec![
            "mcp:dashboard.save:call".into(),
            "mcp:report.save:call".into(),
        ],
    };
    let caps = resolve_caps_with(&store, WS, "bob", &live).await?;
    assert!(
        caps.contains(&"mcp:report.save:call".to_string()),
        "the live built-in bundle resolves"
    );
    assert!(
        caps.contains(&"mcp:ext.custom_tool:call".to_string()),
        "a direct role-subject grant must survive the replace — it is the extension-install path, \
         and it resolves through the role-subject recursion rather than the stored record"
    );
    Ok(())
}
