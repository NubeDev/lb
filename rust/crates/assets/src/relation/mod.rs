//! The generic relation edge — the workspace-internal membership graph (README §6.1 graph
//! model, §6.11; tenancy + files + skills scopes). One shape backs every S4 sharing fact:
//!
//! | `kind`   | `a` → `b`               | meaning                                  |
//! |----------|-------------------------|------------------------------------------|
//! | `share`  | `doc` → `team`          | the doc is shared to the team            |
//! | `link`   | `doc` → `channel`       | the doc is linked into the channel       |
//! | `grant`  | `skill` → `_` (`"ws"`)  | the workspace granted the skill          |
//! | `member` | `team` → `user`         | the user is a member of the team         |
//!
//! Modeling all four as one `(kind, a, b)` edge — rather than four bespoke tables — is the
//! point: the host resolves "may X read doc D?" by a few `related`/`list_related` lookups, and
//! revoking is one `unrelate`. A relation is a *record* at S4 (not a SurrealDB `RELATE` edge);
//! the names are chosen so a later graph-backed projection is a drop-in (files scope open Q).
//!
//! Workspace-namespaced like every record (README §7); raw verbs, no authorization here.
//! One verb per file (FILE-LAYOUT §3).

mod grants;
mod list;
mod model;
mod relate;
mod unrelate;

pub use grants::list_skill_grants;
pub use list::list_related;
pub use model::Relation;
pub use relate::{relate, related};
pub use unrelate::unrelate;

/// The store table all relation edges live in, within a workspace namespace.
pub(crate) const TABLE: &str = "rel";

/// The stable record id for a `(kind, a, b)` edge. `__` separates the parts; the parts
/// themselves never contain `__` in S4 usage (doc/team/channel/skill/user ids are
/// `:`/`/`-delimited), so the key is unambiguous.
pub(crate) fn rel_id(kind: &str, a: &str, b: &str) -> String {
    format!("{kind}__{a}__{b}")
}
