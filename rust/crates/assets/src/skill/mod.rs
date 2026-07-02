//! The skill asset — a versioned, grant-gated workspace asset (README §6.12, §6.16, skills
//! scope). Same store-side shape as a doc, with two additions the model carries: a `version`
//! (a skill is `{id}@{version}`, immutable per version) and the fact that the host only returns
//! it behind a workspace **grant** relation (`grant:skill/{id}` — see `relation`).
//!
//! One verb per file (FILE-LAYOUT §3): the [`Skill`] model, [`put_skill`] (publish a version),
//! [`get_skill`] (load a specific version), [`list_skills`] (all versions of an id). The grant
//! gate is the host's job — these are raw store verbs.

mod core;
mod corpus;
mod get;
mod list;
mod meta;
mod model;
mod put;
mod seed;

pub use core::{
    get_core_skill, is_core, list_core_skill_versions, seed_core_skill, CORE_PREFIX, CORE_SKILLS_NS,
};
pub use get::get_skill;
pub use list::list_skills;
pub use meta::{is_deprecated, set_deprecated, SkillMeta};
pub use model::Skill;
pub use put::put_skill;
pub use seed::seed_core_skills;

/// The store table all skill assets live in, within a workspace namespace.
pub(crate) const TABLE: &str = "skill";

/// The stable record id for skill `id` at `version`: `{id}@{version}`. Each version is its own
/// immutable row, so rollback (§6.4) is just loading a prior version's record.
pub(crate) fn skill_id(id: &str, version: &str) -> String {
    format!("{id}@{version}")
}
