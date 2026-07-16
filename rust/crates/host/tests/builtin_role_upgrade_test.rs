//! **The upgrade path for a SHRINKING built-in role bundle** (auth-caps / builtin-role-freshness).
//!
//! `builtin_role_freshness_test` (in `lb-authz`) pins the resolver mechanism with a synthetic bundle.
//! This test pins the same property one layer up, with lb's **real** `member_role_caps()` resolved
//! through `resolve_caps_live` — the exact function the login mint calls — against a store whose
//! `member` row was written by an OLDER binary. It is the end-to-end half: not "does replace work?"
//! but "does a real member on a real pre-fix workspace actually stop being an admin?"
//!
//! ## Why this test exists
//!
//! On 2026-07-16 the broad `mcp:*.list:call` / `mcp:*.delete:call` wildcards were removed from the
//! member bundle: their `*` spans the `<tool>` half of `<tool>.<verb>`, so they authorized ten
//! `ADMIN_ONLY_CAPS` — `GET /admin/teams` returned 200 with the full roster to a plain member on a
//! live node (`debugging/auth/member-wildcard-satisfies-admin-cap.md`).
//!
//! Removing them from the code fixed **no existing deployment**. Two mechanisms combined:
//!
//! - `ensure_builtin_authz_roles` → `ensure_one` is **create-only**: a workspace seeded by the older
//!   binary keeps its `member` row forever. Nothing rewrites it; there is no migration.
//! - `resolve_caps_with` **unioned** the live bundle on top of that stored row. A union is a
//!   **floor** — it can add a cap to a stale workspace, never remove one.
//!
//! So the stale row's wildcards folded straight back into every member's token and the security fix
//! was inert on precisely the deployments that had the bug. The live verification of the wildcard fix
//! only passed because `make purge-store` had wiped the store first — the exact false green this class
//! produces, and the reason this test asserts against a *deliberately stale* row instead of a fresh one.
//!
//! The fix: for a **built-in** name the live bundle is authoritative and REPLACES the stored record.
//! This test is the proof it holds for the real bundle through the real entry point.

use lb_auth::Principal;
use lb_authz::{grant_assign, role_define, Subject};
use lb_caps::{matches, Action, Request, Surface};
use lb_host::{admin_only_caps, ensure_builtin_authz_roles, resolve_caps_live, ROLE_MEMBER};
use lb_store::{Store, StoreError};

const WS: &str = "acme";

/// The member row exactly as a pre-2026-07-16 binary seeded it: the broad author/viewer wildcards.
/// Verbatim from the bundle at that commit — the point is that this row is what real stores contain.
const STALE_MEMBER_ROW: &[&str] = &[
    "mcp:*.get:call",
    "mcp:*.list:call",
    "mcp:*.write:call",
    "mcp:*.create:call",
    "mcp:*.update:call",
    "mcp:*.delete:call",
    "mcp:*.post:call",
    "store:*:read",
    "store:*:write",
    "mcp:dashboard.save:call",
];

/// `holds_cap`'s logic (not re-exported at the crate root): parse a `surface:resource:action` grant
/// into the request it authorizes and ask the real matcher — i.e. "would this pass Gate 2?".
fn authorizes(caps: &[String], cap: &str) -> bool {
    let mut parts = cap.splitn(3, ':');
    let Some(surface) = parts.next().and_then(Surface::parse) else {
        return false;
    };
    let (Some(resource), Some(action_str)) = (parts.next(), parts.next()) else {
        return false;
    };
    let Some(action) = Action::parse(action_str) else {
        return false;
    };
    matches(caps, &Request::new(WS, surface, resource, action))
}

/// A workspace seeded by the OLD binary, then upgraded: the stale `member` row survives the
/// create-only seed, and a plain member must STILL authorize zero admin-only caps.
///
/// Under the old union this failed with nine: `teams.list`, `roles.list`, `roles.delete`,
/// `grants.list`, `workspace.delete`, `ext.list`, `series.delete`, `nav.delete`, `invite.list`.
#[tokio::test]
async fn a_stale_member_row_cannot_readmit_an_admin_cap_after_upgrade() -> Result<(), StoreError> {
    let store = Store::memory().await?;

    // 1. The pre-fix world: the older binary seeded `member` with the leaky wildcards.
    let stale: Vec<String> = STALE_MEMBER_ROW.iter().map(|s| s.to_string()).collect();
    role_define(&store, WS, ROLE_MEMBER, &stale).await?;

    // 2. The upgrade: the fixed binary boots and seeds. `ensure_one` is create-only, so this must
    //    NOT rewrite the row — the whole point is that the stale row is still there afterwards.
    ensure_builtin_authz_roles(&store, WS).await?;

    // 3. bob is a plain member of that workspace.
    let bob = Subject::User("user:bob".into());
    grant_assign(&store, WS, &bob, &format!("role:{ROLE_MEMBER}")).await?;

    // 4. Log him in through the REAL path (`resolve_caps_live` = `resolve_caps_with(.., &LiveBuiltinRoleCaps)`).
    let caps = resolve_caps_live(&store, WS, "user:bob").await?;

    // The stale row's wildcards must not be readmitted...
    let readmitted: Vec<&String> = caps.iter().filter(|c| c.starts_with("mcp:*.")).collect();
    assert!(
        readmitted.is_empty(),
        "the stale stored row's broad wildcards must NOT resolve into a member's token after \
         upgrade — the live built-in bundle is authoritative, not the row. Readmitted: \
         {readmitted:?}"
    );

    // ...and, the property that actually matters, he must authorize NO admin-only cap.
    let leaked: Vec<String> = admin_only_caps()
        .into_iter()
        .filter(|cap| authorizes(&caps, cap))
        .collect();
    assert!(
        leaked.is_empty(),
        "a plain member on a workspace seeded BEFORE the wildcard fix must authorize no admin-only \
         cap after upgrading. Under the old union the stale row readmitted {} of them: {leaked:#?}\n\
         `ensure_one` is create-only, so no upgrade rewrites the row — if the resolver reads it, the \
         security fix never reaches a real deployment.",
        leaked.len()
    );

    Ok(())
}

/// The other direction, so the test above cannot pass by resolving nothing: the same stale-row member
/// still gets his real authoring reach from the live bundle. A fix that silently emptied a member's
/// caps would satisfy "no admin caps" perfectly.
#[tokio::test]
async fn a_stale_member_row_still_resolves_the_live_authoring_reach() -> Result<(), StoreError> {
    let store = Store::memory().await?;
    let stale: Vec<String> = STALE_MEMBER_ROW.iter().map(|s| s.to_string()).collect();
    role_define(&store, WS, ROLE_MEMBER, &stale).await?;
    ensure_builtin_authz_roles(&store, WS).await?;
    let bob = Subject::User("user:bob".into());
    grant_assign(&store, WS, &bob, &format!("role:{ROLE_MEMBER}")).await?;

    let caps = resolve_caps_live(&store, WS, "user:bob").await?;
    let principal = Principal::routed("user:bob", WS, caps.clone());
    assert!(
        !principal.caps().is_empty(),
        "a member must resolve a real bundle, not an empty one"
    );
    for needed in [
        "mcp:dashboard.save:call",
        "mcp:rules.save:call",
        "mcp:flows.save:call",
        "mcp:ingest.write:call",
        "mcp:nav.resolve:call",
    ] {
        assert!(
            authorizes(&caps, needed),
            "the live member bundle must still grant the authoring cap {needed} — the wildcard \
             removal narrowed the bundle to named verbs, it did not shrink a member's real reach"
        );
    }
    Ok(())
}
