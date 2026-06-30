//! The relation edge shape: a `(kind, a, b)` triple, plus a `pair` field the store can filter
//! on to list all `b`s for a given `(kind, a)`.

use serde::{Deserialize, Serialize};

/// A directed relation edge `a -[kind]-> b`, scoped to a workspace. `pair` is `{kind}__{a}` —
/// a denormalized filter key so `list_related(kind, a)` is one `store::list` by field. `bpair`
/// is `{kind}__{b}` — the inverse key so `list_related_inverse(kind, b)` (backlinks: every doc
/// linking *to* `b`) is likewise one field-equality filter. The store filters on a single
/// `data.<field>`, so each compound key lives in its own column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation {
    pub kind: String,
    pub a: String,
    pub b: String,
    /// Denormalized `{kind}__{a}` so a listing of every `b` for `(kind, a)` is a single
    /// field-equality filter (the store has no compound-key query).
    pub pair: String,
    /// Denormalized `{kind}__{b}` for the inverse listing (every `a` for `(kind, b)`). Defaults
    /// empty for records written before this field existed (document-store scope addition).
    #[serde(default)]
    pub bpair: String,
}

impl Relation {
    pub fn new(kind: impl Into<String>, a: impl Into<String>, b: impl Into<String>) -> Self {
        let kind = kind.into();
        let a = a.into();
        let b = b.into();
        Self {
            pair: format!("{kind}__{a}"),
            bpair: format!("{kind}__{b}"),
            kind,
            a,
            b,
        }
    }
}
