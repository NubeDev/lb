//! The on-disk envelope for a stored value. The host speaks `serde_json::Value`; SurrealDB
//! speaks its own value model. We bridge by wrapping the host JSON under a single concrete
//! `data` field — a typed shape SurrealDB serializes/deserializes cleanly, avoiding the
//! `serde_json::Value` ↔ SurrealDB enum-tag mismatch (debugging/store/
//! content-rejects-serde-json-value.md).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A stored document: the host's JSON value under `data`. The record `id` lives outside this
/// struct (SurrealDB manages it), so deserialization never trips on the `id` Thing type.
#[derive(Serialize, Deserialize)]
pub(crate) struct Record {
    pub data: Value,
}
