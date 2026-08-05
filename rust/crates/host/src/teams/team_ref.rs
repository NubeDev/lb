//! **Normalising a team reference.** One responsibility: turn whatever form a `share` edge stored a
//! team in into the bare id the `member` edges are actually keyed by.
//!
//! ## Why this exists (2026-08-05)
//!
//! A `team`-visible record is read via two hops: `record -[share]-> team`, then
//! `team -[member]-> user`. The **`member`** edges are always keyed by the BARE team id (`app`) —
//! that is what `POST /teams/{team}/members` writes and what `teams/delete.rs` walks. But the
//! **`share`** edge stores whatever the caller sent, and callers disagree:
//!
//! ```text
//! nav admin UI      → share edge "test"        member lookup "test"       ✅ resolves
//! setup wizard      → share edge "team:app"    member lookup "team:app"   ❌ no such team
//! ```
//!
//! The prefixed form silently resolved to *no members*, so a nav shared through the onboarding wizard
//! never applied to anyone. The failure is invisible from every angle that matters: the team exists,
//! the membership exists, the nav exists, `GET /navs/{id}/shares` proudly returns the team — and
//! `nav.resolve` still falls through to `fallback`, handing the member the full built-in rail. Nothing
//! errors and nothing logs.
//!
//! Normalising at the READ side (here) rather than only fixing the writer is deliberate: it heals
//! edges already written into live stores, and it means a future caller that sends the prefixed form
//! degrades to "works" instead of "silently grants nothing". The `nav:`-prefix tolerance in
//! `nav::resolve::readable_nav` is the same idea and the precedent for it.
//!
//! This normalises the REFERENCE only. It is not an authorization decision — the membership check
//! still runs, unchanged, against the real edges.

/// The `team:` subject prefix. A grant SUBJECT is legitimately `team:app` (see `authz::Subject`), so
/// the prefixed form is not "wrong" everywhere — it is only wrong as a `member`-edge key.
const TEAM_PREFIX: &str = "team:";

/// The bare team id a `member` edge is keyed by. Accepts `app` or `team:app`; returns `app`.
pub fn bare_team(team: &str) -> &str {
    team.strip_prefix(TEAM_PREFIX).unwrap_or(team)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_forms_normalise_to_the_bare_id() {
        assert_eq!(bare_team("app"), "app");
        assert_eq!(bare_team("team:app"), "app");
    }

    /// Only a LEADING `team:` is stripped, and only once — a team legitimately named `team:x` after
    /// one strip must not be stripped again, and an id merely CONTAINING the word is untouched.
    #[test]
    fn strips_one_leading_prefix_only() {
        assert_eq!(bare_team("team:team:app"), "team:app");
        assert_eq!(bare_team("my-team:app"), "my-team:app");
        assert_eq!(bare_team("ops-team"), "ops-team");
        assert_eq!(bare_team(""), "");
    }
}
