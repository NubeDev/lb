//! The document asset shape (README §6.12, files scope).
//!
//! A doc is *state*: content + the metadata the host needs to resolve a read's visibility
//! (owner, visibility class). It lives in the workspace namespace addressed by `doc:{id}`.
//! `content` is an opaque string at S4 (docs are text — scope docs, skill bodies); the field
//! is the seam a real bucket backend fills with bytes later (files scope non-goal).

use serde::{Deserialize, Serialize};

/// How a doc may be reached, *within* its workspace. The host resolves this on every read:
/// `Private` → owner only; `Shared`/`Linked` visibility is carried by relation records (a doc
/// shared to a team has a `share` relation), so this enum is the *base* class — a `Private`
/// doc with a `share` relation is reachable by that team. Kept tiny on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Visibility {
    /// Reachable only by the owner (plus any team/channel the owner explicitly shared/linked to).
    Private,
}

impl Default for Visibility {
    fn default() -> Self {
        Visibility::Private
    }
}

/// A document asset. `id` is workspace-unique and stable (re-`put` upserts the same row).
/// `owner` is the normalized principal who created it (`user:…`). `ts` is a caller-injected
/// logical timestamp (testing §3 determinism — no wall-clock in the crate).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Doc {
    pub id: String,
    pub owner: String,
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub visibility: Visibility,
    pub ts: u64,
}

impl Doc {
    /// Build a private doc owned by `owner`. Explicit (no `Default`) so every field is a
    /// deliberate choice at the call site.
    pub fn new(
        id: impl Into<String>,
        owner: impl Into<String>,
        title: impl Into<String>,
        content: impl Into<String>,
        ts: u64,
    ) -> Self {
        Self {
            id: id.into(),
            owner: owner.into(),
            title: title.into(),
            content: content.into(),
            visibility: Visibility::Private,
            ts,
        }
    }
}
