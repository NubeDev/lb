//! Read a SurrealDB `NULL`/`NONE` column into `Option<T>`.
//!
//! SurrealDB has two empties — `NULL` (written, no value) and `NONE` (never written) — and the
//! SurrealDB 3 value bridge hands both to serde as a **unit**, not as `null`. Serde's own
//! `Option<T>` does not accept a unit there, so a nullable column that round-trips through JSON
//! fails to decode: `lb_tags::Applied::expires` (`Option<u64>`, bound as `json!(None)` by
//! `tag.add`) came back as `invalid type: unit value, expected u64`.
//!
//! `#[serde(default)]` does not cover this — that is for an ABSENT key, and here the key is present
//! and empty. Two different bugs; only one of them survives an engine upgrade.
//!
//! Use on any `Option<T>` field read back from a row:
//!
//! ```ignore
//! #[serde(default, deserialize_with = "lb_store::null_as_none")]
//! pub expires: Option<u64>,
//! ```

use serde::{Deserialize, Deserializer};

/// Deserialize `Option<T>`, treating SurrealDB's unit-shaped `NULL`/`NONE` as `None`.
pub fn null_as_none<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    // `untagged` tries each arm in order, so a unit matches `Empty` and anything else is decoded as
    // `T` — which keeps a real value working exactly as before.
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NullOr<T> {
        Value(T),
        Empty(()),
    }

    Ok(match NullOr::<T>::deserialize(d)? {
        NullOr::Value(v) => Some(v),
        NullOr::Empty(()) => None,
    })
}
