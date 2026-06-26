//! The skill asset shape (README §6.12, skills scope).
//!
//! A skill is a versioned instruction/recipe asset. `id` is the stable skill name; `version`
//! makes `{id}@{version}` immutable (a change is a new version, rollback loads a prior one,
//! §6.4). `skill_key` is the denormalized `id` the store filters on to list every version.

use serde::{Deserialize, Serialize};

/// A versioned skill asset. Loaded by an AI agent only when the workspace has granted the skill
/// (the host resolves a `grant:skill/{id}` relation — skills scope). `body` is the
/// instruction/recipe text (free-form at S4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub body: String,
    pub ts: u64,
    /// Denormalized `id` so `list_skills(id)` is one field-equality filter over every version.
    #[serde(default)]
    pub skill_key: String,
}

impl Skill {
    pub fn new(
        id: impl Into<String>,
        version: impl Into<String>,
        author: impl Into<String>,
        description: impl Into<String>,
        body: impl Into<String>,
        ts: u64,
    ) -> Self {
        let id = id.into();
        Self {
            skill_key: id.clone(),
            id,
            version: version.into(),
            author: author.into(),
            description: description.into(),
            body: body.into(),
            ts,
        }
    }
}
