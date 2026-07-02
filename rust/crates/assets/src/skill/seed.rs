//! Seed the embedded core-skill corpus into the reserved namespace at boot (core-skills scope,
//! "seeded at boot, versioned by release"). For each embedded `SKILL.md` this writes
//! `skill:core.<name>@<version>` if absent — idempotent, because immutable versions make a re-seed a
//! no-op. A node upgrade bumps `version` and seeds the NEW versions; the old ones remain for
//! rollback (§6.4). The `node` boot is the single caller (the boot seeder is the only writer).

use lb_store::{Store, StoreError};

use super::core::seed_core_skill;
use super::corpus::core_skill_corpus;
use super::model::Skill;
use super::CORE_PREFIX;

/// Seed every embedded core skill at `version` (the node build version). Returns the ids seeded (for
/// the boot log). Idempotent: seeding the same version twice writes each record once. `ts` is the
/// caller-injected boot timestamp (no wall-clock in the crate — testing §3).
pub async fn seed_core_skills(
    store: &Store,
    version: &str,
    ts: u64,
) -> Result<Vec<String>, StoreError> {
    let mut seeded = Vec::new();
    for entry in core_skill_corpus() {
        let id = format!("{CORE_PREFIX}{}", entry.name);
        let skill = Skill::new(
            &id,
            version,
            "core", // author: the platform, not a workspace member.
            entry.description,
            entry.body,
            ts,
        );
        seed_core_skill(store, &skill).await?;
        seeded.push(id);
    }
    Ok(seeded)
}
