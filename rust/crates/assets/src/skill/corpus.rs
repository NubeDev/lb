//! The embedded core-skill corpus — a build-time snapshot of `docs/skills/*/SKILL.md` (core-skills
//! scope). `build.rs` parses each SKILL.md's frontmatter (`name`/`description`), strips it, flags
//! repo-relative links, and generates the [`CORE_SKILLS`] slice `include!`d below. This is the ONLY
//! source the boot seeder writes from — the corpus is data compiled into the binary, versioned by
//! the node build, so a node upgrade ships a new corpus and re-seeds new immutable versions.

include!(concat!(env!("OUT_DIR"), "/core_skills_corpus.rs"));

/// One embedded core skill: its `name` (→ id `core.<name>`), one-line `description`, and body.
pub struct CoreSkill {
    pub name: &'static str,
    pub description: &'static str,
    pub body: &'static str,
}

/// The embedded corpus as structured entries, in a stable (sorted) order.
pub fn core_skill_corpus() -> impl Iterator<Item = CoreSkill> {
    CORE_SKILLS
        .iter()
        .map(|(name, description, body)| CoreSkill {
            name,
            description,
            body,
        })
}
