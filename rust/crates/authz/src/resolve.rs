//! [`resolve_caps`] — the session projection: compute the capability set a login token should carry
//! for `user` in workspace `ws` (authz-grants scope). The token is a **cached projection** of this;
//! the enforcement gates (`lb_caps::check`) read the token, not this function, on the hot path.
//!
//! ```text
//!   caps = ( direct user grants
//!          ∪ the user's roles' caps
//!          ∪ for each team the user is a member of:  team's grants ∪ team's roles' caps )
//! ```
//!
//! **The freshness asymmetry (load-bearing).** Because the token caches this, a Gate-2 capability
//! change (grant/revoke a cap or role, add/remove a *team-inherited* cap) only takes effect on the
//! next **re-mint** — bounded by the token TTL. A Gate-3 membership/visibility change is **live**
//! (the S4 relation is re-resolved per read). So "remove Bob from the team" drops his access to
//! resources shared to that team *immediately* (Gate 3) but leaves team-inherited caps in his
//! current token until it expires. This function is the Gate-2 (cached) half; do not assume
//! revocation is uniformly instant. See the resolver's tests and the admin-crud revoke seam.
//!
//! **Built-in role freshness (builtin-role-freshness scope).** A built-in role (`viewer`/`member`/
//! `workspace-admin`) is seeded idempotently — its stored record is written only when ABSENT — so a
//! row seeded before a new built-in cap was added stays FROZEN at the old set. Reading only the
//! stored record (the pre-fix behaviour) meant a new built-in cap never reached an already-seeded
//! workspace's tokens. [`resolve_caps_with`]/[`resolve_subject_caps_with`] take a [`BuiltinRoleCaps`]
//! callback; a host caller passes the LIVE `*_role_caps()` bundles, which the resolver UNIONS on top
//! of the stored record for granted built-in roles. So a new built-in cap takes effect the moment
//! code ships — no re-seed, no version bump — while custom roles and direct role-subject grants are
//! untouched. The zero-arg [`resolve_caps`]/[`resolve_subject_caps`] (with [`NoBuiltinRoleCaps`])
//! preserve the raw stored-row fold for this crate's own tests.

use std::collections::BTreeSet;

use lb_assets::list_related;
use lb_store::{Store, StoreError};

use crate::grant::grant_list;
use crate::role::role_caps;
use crate::subject::Subject;
use crate::team::team_list;
use crate::MEMBER;

/// The live-built-in-role cap source injected into the resolver (builtin-role-freshness scope).
///
/// `resolve_subject_caps`/`resolve_caps` live in this pure crate, which must NOT depend on the host
/// (where the authoritative `*_role_caps()` bundles live). But the resolver is the one chokepoint
/// every cap mint funnels through, and reading only the *stored* role record is the footgun: a
/// built-in role row is seeded idempotently (written only when absent), so once a workspace is
/// seeded its `member`/`workspace-admin` rows are FROZEN at that code's cap set. Adding a new
/// built-in cap (e.g. `mcp:report.save:call`) to `AUTHOR_CAPS` never reaches an already-seeded
/// workspace's tokens — `role_caps` reads the stale row.
///
/// The fix is dependency injection of the ONE thing this crate can't know: the **live** cap bundle
/// for a built-in role name. Callers that can see the host pass `Some(&BuiltinRoleCaps::live)`;
/// pure-authz callers/tests pass `None` (current behaviour — read the stored row only). When `Some`,
/// the resolver UNIONS the live bundle for a granted built-in role ON TOP of the stored record, so a
/// new built-in cap takes effect the moment code ships, with no re-seed and no version bump. Custom
/// roles are unaffected (they have no live bundle) and the stored record is still read (an admin's
/// `grant_assign(Subject::Role(name), cap)` additions are honoured). See
/// `docs/debugging/authz/builtin-role-row-frozen-stale-on-new-caps.md`.
pub trait BuiltinRoleCaps: Send + Sync {
    /// The live caps for built-in role `name`, or `None` if `name` is not a built-in role.
    fn live_caps(&self, name: &str) -> Option<Vec<String>>;
}

/// A `BuiltinRoleCaps` that knows no built-in roles — the default, preserving the pre-fix behaviour
/// (the resolver reads only the stored role record). Used by `lb-authz`-internal tests and any caller
/// that genuinely wants the raw stored-row fold.
pub struct NoBuiltinRoleCaps;

impl BuiltinRoleCaps for NoBuiltinRoleCaps {
    fn live_caps(&self, _name: &str) -> Option<Vec<String>> {
        None
    }
}

/// Resolve `user`'s effective caps in workspace `ws` as the union described above, reading only the
/// stored role records (no live built-in caps). Deduplicated and sorted (a `BTreeSet`) so the minted
/// token is deterministic (testing §3 — no incidental ordering). Equivalent to
/// [`resolve_caps_with`] with [`NoBuiltinRoleCaps`]; kept as the zero-arg entry point for callers
/// that don't inject live caps (and for the resolver's own tests).
pub async fn resolve_caps(store: &Store, ws: &str, user: &str) -> Result<Vec<String>, StoreError> {
    resolve_caps_with(store, ws, user, &NoBuiltinRoleCaps).await
}

/// [`resolve_caps`] with an injected [`BuiltinRoleCaps`] — the entry point a host caller uses so a
/// new built-in cap reaches already-seeded workspaces without a re-seed (builtin-role-freshness).
pub async fn resolve_caps_with(
    store: &Store,
    ws: &str,
    user: &str,
    builtins: &dyn BuiltinRoleCaps,
) -> Result<Vec<String>, StoreError> {
    let mut caps: BTreeSet<String> = BTreeSet::new();

    // Direct user grants (+ any roles they name).
    resolve_subject_caps_with(
        store,
        ws,
        &Subject::User(user.to_string()),
        builtins,
        &mut caps,
    )
    .await?;

    // Team-inherited: for every team the user is a member of, fold the team's grants + roles.
    for team in team_list(store, ws).await? {
        let members = list_related(store, ws, MEMBER, &team.team).await?;
        if members.iter().any(|m| m == user) {
            resolve_subject_caps_with(
                store,
                ws,
                &Subject::Team(team.team.clone()),
                builtins,
                &mut caps,
            )
            .await?;
        }
    }

    Ok(caps.into_iter().collect())
}

/// Resolve an arbitrary `subject`'s **direct** caps in workspace `ws` — direct grants plus the
/// expansion of any `role:<name>` grant into that role's bundled caps — into `caps` (api-keys
/// scope), reading only the stored role records. This is the generalized inner of [`resolve_caps`]:
/// a user folds this for itself AND its teams (team membership is a *user* concept); an API key
/// (`Subject::Key`) calls this directly — it has direct grants + roles but **no team-membership
/// edge** (a key joins no teams in v1).
///
/// **Why a key must call THIS and not `resolve_caps(&str)`:** `resolve_caps` wraps its `user` arg in
/// `Subject::User`, so passing `"key:…"` would build `Subject::User("key:…")` → resolve to **zero**
/// caps → silently deny everything. A key passes its own `Subject::Key(id)` here.
pub async fn resolve_subject_caps(
    store: &Store,
    ws: &str,
    subject: &Subject,
    caps: &mut BTreeSet<String>,
) -> Result<(), StoreError> {
    resolve_subject_caps_with(store, ws, subject, &NoBuiltinRoleCaps, caps).await
}

/// [`resolve_subject_caps`] with an injected [`BuiltinRoleCaps`] (builtin-role-freshness scope).
pub async fn resolve_subject_caps_with(
    store: &Store,
    ws: &str,
    subject: &Subject,
    builtins: &dyn BuiltinRoleCaps,
    caps: &mut BTreeSet<String>,
) -> Result<(), StoreError> {
    for cap in grant_list(store, ws, subject).await? {
        match cap.strip_prefix("role:") {
            Some(role) => {
                // A role contributes BOTH its record caps AND any caps granted directly to the role
                // subject (`grant_assign(Subject::Role(name), cap)`). The latter is how an installed
                // extension's page tools reach every holder of the role without touching a built-in
                // role's (immutable) record — an ordinary grant, per authz-grants scope. It is
                // resolved by the `role_subject` recursion BELOW, not here.
                //
                // BUILTIN-ROLE FRESHNESS: for a BUILT-IN name the live bundle (host `*_role_caps()`)
                // is authoritative and REPLACES the stored record — the row is not consulted at all.
                // For a custom role (`live_caps` → `None`) the stored record is authoritative, exactly
                // as before.
                //
                // This was a UNION until 2026-07-16, which made the live bundle a *floor* — it could
                // add a cap to a stale workspace but never remove one. That is fine while bundles only
                // ever grow; it is a security hole the moment one shrinks. When the broad `mcp:*.list`
                // / `mcp:*.delete` wildcards were removed from the member bundle (they authorized ten
                // admin-only caps — see `debugging/auth/member-wildcard-satisfies-admin-cap.md`), every
                // workspace seeded by the older binary kept them: `ensure_one` is create-only, so the
                // stale row survives the upgrade and the union folded its wildcards straight back into
                // every member's token. The fix was inert on exactly the deployments that had the bug.
                //
                // Replace loses nothing the union bought: the `Subject::Role` grant path that motivated
                // it is the recursion BELOW, not the record, so an extension's
                // `grant_assign(Subject::Role("member"), cap)` still reaches every member.
                //
                // It DOES mean a `roles.define("member", ...)` no longer affects what a member
                // resolves (`roles.define` has no built-in guard today — only `roles.delete` does).
                // That is the intended posture, not a casualty: a built-in bundle is lb's policy, and
                // an admin redefining `member` to widen it is precisely the escalation this module
                // exists to prevent. No-widening already stops them adding a cap they lack, and an
                // admin who wants to grant caps to every member has the supported path —
                // `grant_assign(Subject::Role("member"), cap)` — which still works. Custom roles are
                // entirely unaffected. `live_builtin_caps_replace_the_stale_role_row` pins both
                // directions.
                match builtins.live_caps(role) {
                    Some(live) => {
                        for rc in live {
                            caps.insert(rc);
                        }
                    }
                    None => {
                        for rc in role_caps(store, ws, role).await? {
                            caps.insert(rc);
                        }
                    }
                }
                let role_subject = Subject::Role(role.to_string());
                if &role_subject != subject {
                    Box::pin(resolve_subject_caps_with(
                        store,
                        ws,
                        &role_subject,
                        builtins,
                        caps,
                    ))
                    .await?;
                }
            }
            None => {
                caps.insert(cap);
            }
        }
    }
    Ok(())
}
