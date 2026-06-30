//! The document asset shape (README §6.12, files + document-store scopes).
//!
//! A doc is *state*: content + the metadata the host needs to resolve a read's visibility
//! (owner, visibility class). It lives in the workspace namespace addressed by `doc:{id}`.
//! `content` is an opaque string — typed by `content_type` so the document-store slice can
//! carry **raw markdown** (`Markdown`) alongside the legacy opaque `Text` (document-store
//! scope move 1: "type the content"). The field stays the seam a real bucket backend fills
//! with bytes later (files scope non-goal); markdown is still text here.

use serde::{Deserialize, Serialize};

/// How a doc's `content` is to be interpreted. `Text` is the S4 legacy opaque string;
/// `Markdown` is the document-store typed content (raw markdown — the renderer, not the
/// store, turns it into HTML; document-store scope "server-side rendering is a non-goal").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    #[default]
    Text,
    Markdown,
}

/// How a doc may be reached, *within* its workspace. The host resolves this on every read:
/// `Private` → owner only; `Shared`/`Linked`/`User` reachability is carried by relation
/// records (a doc shared to a team has a `share` relation), so this enum is the *base* class
/// — a `Private` doc with a `share` relation is reachable by that team. Kept tiny on purpose.
/// `User` flags a doc shared to an individual (document-store scope: the `user` subject).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Visibility {
    /// Reachable only by the owner (plus any team/channel/user the owner explicitly shared/linked to).
    #[default]
    Private,
}

/// A document asset. `id` is workspace-unique and stable (re-`put` upserts the same row).
/// `owner` is the normalized principal who created it (`user:…`). `ts` is a caller-injected
/// logical timestamp (testing §3 determinism — no wall-clock in the crate). `content_type`
/// types `content`; `tags` is a flat list for v1 discovery (tags-scope is the richer layer).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Doc {
    pub id: String,
    pub owner: String,
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub content_type: ContentType,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub visibility: Visibility,
    pub ts: u64,
}

impl Doc {
    /// Build a private text doc owned by `owner`. Explicit (no `Default`) so every field is a
    /// deliberate choice at the call site. Use [`Doc::with_content_type`] / [`Doc::with_tags`]
    /// to set the document-store markdown fields.
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
            content_type: ContentType::Text,
            tags: Vec::new(),
            visibility: Visibility::Private,
            ts,
        }
    }

    /// Builder: set the content type (markdown docs carry raw markdown).
    pub fn with_content_type(mut self, ct: ContentType) -> Self {
        self.content_type = ct;
        self
    }

    /// Builder: set the discovery tags.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
}
