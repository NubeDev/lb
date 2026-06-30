//! [`resolve_caps_sourced`] — the **provenance-tagging wrapper** over the shipped
//! [`resolve_caps`](crate::resolve_caps) / [`resolve_subject_caps`](crate::resolve_subject_caps)
//! fold (access-console scope). It runs the *same* union the session mint computes, but instead of
//! collapsing each contributed cap into a flat set it records **where each cap came from** — `Direct`
//! (a plain grant on the subject) / `Role(name)` (expanded from a `role:<name>` grant the subject
//! holds) / `Team(name)` (inherited through a team the user is a member of) — so an admin sees not
//! just *that* a subject can do X but *why*, and knows which grant to edit.
//!
//! **This is not a parallel resolver.** It is the one shipped fold, re-run with a tag accumulator
//! instead of a `BTreeSet`. The cap *set* it yields is therefore byte-for-byte what
//! [`resolve_caps`] yields for the same subject — there is no resolver↔mint drift (the
//! `resolve_sourced_eq_resolve` cross-check test pins that). Provenance is a *view* over the one
//! resolver, never a second implementation.
//!
//! CapSource granularity mirrors the scope's three variants exactly: a user's own direct grant →
//! `Direct`; a user's own `role:` grant expanded → `Role(name)`; anything inherited through a team
//! the user belongs to (the team's direct grant *or* the team's `role:` grant) → `Team(name)`. A
//! cap contributed by more than one edge carries every distinct source.

use std::collections::BTreeMap;

use lb_assets::list_related;
use lb_store::{Store, StoreError};
use serde::{Deserialize, Serialize};

use crate::grant::grant_list;
use crate::role::role_caps;
use crate::subject::Subject;
use crate::team::team_list;
use crate::MEMBER;

/// Where a resolved cap came from — the provenance tag the access console shows beside each cap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum CapSource {
    /// A plain `mcp:…:call` (or `store:`/`bus:`/…) grant directly on the subject.
    Direct,
    /// Expanded from a `role:<name>` grant the subject itself holds.
    Role { name: String },
    /// Inherited through `team` — the user is a member of `team`, which held this cap (directly or
    /// via one of the team's own roles).
    Team { name: String },
}

/// One resolved cap plus the distinct edges that contributed it (sorted cap-first for determinism).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcedCap {
    pub cap: String,
    pub source: Vec<CapSource>,
}

/// Resolve `user`'s effective caps in workspace `ws` WITH provenance — the sourced twin of
/// [`resolve_caps`](crate::resolve_caps). Same fold (direct ∪ roles ∪ team-inherited), each cap
/// tagged with its contributing edge(s). The cap set equals `resolve_caps(store, ws, user)`.
pub async fn resolve_caps_sourced(
    store: &Store,
    ws: &str,
    user: &str,
) -> Result<Vec<SourcedCap>, StoreError> {
    let mut acc: BTreeMap<String, Vec<CapSource>> = BTreeMap::new();
    // The user's own direct grants + their roles.
    fold_subject(
        store,
        ws,
        &Subject::User(user.to_string()),
        Ctx::UserDirect,
        &mut acc,
    )
    .await?;
    // Team-inherited: for every team the user is a member of, fold the team's grants + roles. Same
    // membership walk `resolve_caps` performs (a membership/visibility relation is the live edge).
    for team in team_list(store, ws).await? {
        let members = list_related(store, ws, MEMBER, &team.team).await?;
        if members.iter().any(|m| m == user) {
            fold_subject(
                store,
                ws,
                &Subject::Team(team.team.clone()),
                Ctx::TeamInherited(team.team.clone()),
                &mut acc,
            )
            .await?;
        }
    }
    Ok(finalize(acc))
}

/// Resolve an arbitrary `subject`'s **direct** caps (grants + role expansion) WITH provenance — the
/// sourced twin of [`resolve_subject_caps`](crate::resolve_subject_caps). Used for a `key:`/`team:`/
/// `role:` subject (no team-membership edge) and as the inner of [`resolve_caps_sourced`]. Every
/// contributed cap is tagged `Direct` (plain grant) or `Role(name)` (expanded from a `role:` grant).
pub async fn resolve_subject_caps_sourced(
    store: &Store,
    ws: &str,
    subject: &Subject,
) -> Result<Vec<SourcedCap>, StoreError> {
    let mut acc: BTreeMap<String, Vec<CapSource>> = BTreeMap::new();
    fold_subject(store, ws, subject, Ctx::UserDirect, &mut acc).await?;
    Ok(finalize(acc))
}

/// The context a fold runs under — `UserDirect` tags the subject's own grants/roles; `TeamInherited`
/// re-tags everything contributed through that team as `Team(name)` (the team is the inheritance
/// edge the admin would edit to change this cap).
#[derive(Clone)]
enum Ctx {
    UserDirect,
    TeamInherited(String),
}

/// Fold `subject`'s grants into `acc`, mirroring [`resolve_subject_caps`](crate::resolve_subject_caps)
/// exactly (same `grant_list` → `role:`-expand loop) but tagging each contributed cap with its
/// source under the current [`Ctx`]. A cap contributed more than once keeps every distinct source.
async fn fold_subject(
    store: &Store,
    ws: &str,
    subject: &Subject,
    ctx: Ctx,
    acc: &mut BTreeMap<String, Vec<CapSource>>,
) -> Result<(), StoreError> {
    for cap in grant_list(store, ws, subject).await? {
        match cap.strip_prefix("role:") {
            Some(role) => {
                for rc in role_caps(store, ws, role).await? {
                    push_source(acc, rc, source_for_role(&ctx, role));
                }
            }
            None => push_source(acc, cap, source_for_direct(&ctx)),
        }
    }
    Ok(())
}

/// The provenance tag for a plain grant under `ctx` (`Direct` for the subject's own, `Team` inherited).
fn source_for_direct(ctx: &Ctx) -> CapSource {
    match ctx {
        Ctx::UserDirect => CapSource::Direct,
        Ctx::TeamInherited(t) => CapSource::Team { name: t.clone() },
    }
}

/// The provenance tag for a cap expanded from a `role:<name>` grant under `ctx` (`Role` for the
/// subject's own role, `Team` when the role is inherited through a team — the team edge is the
/// actionable one in that case).
fn source_for_role(ctx: &Ctx, role: &str) -> CapSource {
    match ctx {
        Ctx::UserDirect => CapSource::Role {
            name: role.to_string(),
        },
        Ctx::TeamInherited(t) => CapSource::Team { name: t.clone() },
    }
}

/// Record `src` for `cap`, deduped against the cap's existing sources (the same grant reaching the
/// same cap through two teams records two `Team` edges, not two identical ones).
fn push_source(acc: &mut BTreeMap<String, Vec<CapSource>>, cap: String, src: CapSource) {
    let entry = acc.entry(cap).or_default();
    if !entry.contains(&src) {
        entry.push(src);
    }
}

/// Collapse the map into the deterministic `Vec<SourcedCap>` (cap-sorted via `BTreeMap`; sources in
/// insertion order, deduped).
fn finalize(acc: BTreeMap<String, Vec<CapSource>>) -> Vec<SourcedCap> {
    acc.into_iter()
        .map(|(cap, source)| SourcedCap { cap, source })
        .collect()
}
