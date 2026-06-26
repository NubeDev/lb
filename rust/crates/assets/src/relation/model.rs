//! The relation edge shape: a `(kind, a, b)` triple, plus a `pair` field the store can filter
//! on to list all `b`s for a given `(kind, a)`.

use serde::{Deserialize, Serialize};

/// A directed relation edge `a -[kind]-> b`, scoped to a workspace. `pair` is `{kind}__{a}` —
/// a denormalized filter key so `list_related(kind, a)` is one `store::list` by field (the
/// store filters on a single `data.<field>`, so the compound key lives in one column).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation {
    pub kind: String,
    pub a: String,
    pub b: String,
    /// Denormalized `{kind}__{a}` so a listing of every `b` for `(kind, a)` is a single
    /// field-equality filter (the store has no compound-key query).
    pub pair: String,
}

impl Relation {
    pub fn new(kind: impl Into<String>, a: impl Into<String>, b: impl Into<String>) -> Self {
        let kind = kind.into();
        let a = a.into();
        Self {
            pair: format!("{kind}__{a}"),
            kind,
            a,
            b: b.into(),
        }
    }
}
