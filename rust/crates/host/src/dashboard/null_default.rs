//! `null_default` — the serde adapter every dashboard record type defaults through.
//!
//! Its own file because all three of `model.rs`, `cell.rs` and `binding.rs` need it and none of them
//! owns it: it is a serde concern, not a record shape.

use serde::Deserialize;

/// Deserialize a defaulted field tolerating an explicit JSON `null` (AI callers emit `"title": null`
/// where a human omits the key — live, two `dashboard.save` turns died on `invalid type: null,
/// expected a string`). `#[serde(default, deserialize_with = "null_default")]` alone only covers the ABSENT key; this covers both.
pub(super) fn null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + serde::Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}
