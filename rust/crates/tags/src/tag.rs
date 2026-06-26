//! The shared, typed **tag node** — `tag:[key, value]` (composite record ID). Deterministic and
//! deduplicated: every entity in a workspace that carries `region:eu` points at the *same* node, so
//! traversal is a graph hop both directions, never a scan (tags scope). Constructed, never looked up.
//!
//! A tag is NOT a string — `value` may be a `string`, `number`, `datetime`, etc. (typed in the
//! composite ID), so range/temporal/geo queries are possible. Tag nodes are per-workspace (the hard
//! wall): `tag:['region','eu']` in ws-A and ws-B are distinct records in distinct namespaces.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The tag node table.
pub const TAG_TABLE: &str = "tag";

/// A typed tag: a `key` (always a string identifier) and a typed `value`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tag {
    pub key: String,
    /// The typed value — string / number / datetime / etc. Kept as a JSON value so the composite
    /// record id preserves its type (a numeric `temp_threshold:80` indexes for range queries).
    pub value: Value,
}

impl Tag {
    pub fn new(key: impl Into<String>, value: Value) -> Self {
        Self {
            key: key.into(),
            value,
        }
    }

    /// The composite record id `[key, value]` — the deterministic, deduplicated node identity.
    pub fn record_id(&self) -> [Value; 2] {
        [Value::String(self.key.clone()), self.value.clone()]
    }
}
