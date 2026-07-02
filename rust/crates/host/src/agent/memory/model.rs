//! The agent-memory record + its scope (agent-memory scope). One **fact** per record in the proven
//! MEMORY.md shape: a one-line `description` for the derived index, a markdown `body` (the fact), a
//! `kind` taxonomy, and `updated_at`/`updated_by` provenance. Keyed `{ws, scope, slug}` — `ws` is
//! the namespace (the hard wall), `{scope, slug}` the composite record id.
//!
//! **The scope is derived from the principal, never an argument** (the member wall): a `member`
//! scope resolves to `member:{authenticated-user}` — a run under user U can only ever read/write
//! `workspace` + `member:U`, structurally never `member:V`. This module holds the shape + the two
//! bounds (`description` ≤ 120, `body` ≤ 8 KB); the resolver owns turning a principal into a scope.

use serde::{Deserialize, Serialize};

/// Max `description` length (chars) — the index line stays short (context tax bound, scope decided).
pub const MAX_DESCRIPTION: usize = 120;
/// Max `body` length (bytes) — a fact, not a document (scope decided: 8 KB).
pub const MAX_BODY: usize = 8 * 1024;

/// The taxonomy a fact is filed under (the proven MEMORY.md `type`). Serialized lowercase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryKind {
    User,
    Feedback,
    Project,
    Reference,
}

impl MemoryKind {
    /// Parse the wire string; `None` for an unknown kind (the verb rejects it as `BadInput`).
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "user" => Some(Self::User),
            "feedback" => Some(Self::Feedback),
            "project" => Some(Self::Project),
            "reference" => Some(Self::Reference),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Feedback => "feedback",
            Self::Project => "project",
            Self::Reference => "reference",
        }
    }
}

/// A memory scope inside a workspace: shared (`workspace`) or private to one member
/// (`member:{user}`). Constructed only by the resolver from the authenticated principal (member) or
/// as the workspace scope — never parsed from a caller argument (the member wall).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryScope {
    /// Shared memory every member's runs see.
    Workspace,
    /// Private memory for one member (`user:ada` → `member:user:ada`).
    Member(String),
}

impl MemoryScope {
    /// The stable string used as the first element of the composite record id (`workspace` |
    /// `member:{user}`). This is what walls one member's memory from another's.
    pub fn key(&self) -> String {
        match self {
            Self::Workspace => "workspace".to_string(),
            Self::Member(user) => format!("member:{user}"),
        }
    }
}

/// One durable memory fact. `scope`/`slug` are the composite id parts; the rest is the payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    /// The scope key (`workspace` | `member:{user}`) — denormalized onto the row for listing.
    pub scope: String,
    /// The stable fact slug within its scope (`staging-db-readonly`).
    pub slug: String,
    /// One-line, index-facing summary (≤ [`MAX_DESCRIPTION`] chars).
    pub description: String,
    /// The fact itself, markdown (≤ [`MAX_BODY`] bytes).
    pub body: String,
    /// The taxonomy.
    pub kind: MemoryKind,
    /// Caller-injected logical timestamp of the last write (no wall-clock — testing §3).
    pub updated_at: u64,
    /// The principal `sub` that last wrote this fact (provenance / audit).
    pub updated_by: String,
}
