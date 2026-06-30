//! The binary-asset shape (document-store scope). An `asset:{ws}:{id}` record: opaque bytes
//! + the metadata the host needs to gate a read (owner, mime) and to bound size. Like a doc,
//! it is *state*, owned by the creating principal, workspace-namespaced (README §7).

use serde::{Deserialize, Serialize};

/// The marker a deleted asset carries — read back as `None` by [`crate::asset::get_asset`]
/// (mirrors the relation tombstone discipline: a delete is an append-style state change that
/// syncs idempotently, not a row that vanishes under a peer — §6.8).
pub(crate) const TOMBSTONE: &str = "__deleted__";

/// A binary asset. `id` is workspace-unique and stable (re-`put` upserts the same row).
/// `bytes` is the raw payload; `mime` is the caller-supplied content type (the store does not
/// sniff it). `ts` is a caller-injected logical timestamp (no wall-clock in the crate).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    pub id: String,
    pub owner: String,
    pub mime: String,
    /// Raw payload, base64-transparent over serde_json (`Vec<u8>` serializes as a string). The
    /// host bounds the length before `put_asset`; the store holds it inline (record value).
    #[serde(with = "serde_bytes_base64")]
    pub bytes: Vec<u8>,
    pub ts: u64,
}

impl Asset {
    /// Build an asset owned by `owner`. Explicit (no `Default`) so every field is a deliberate
    /// choice at the call site.
    pub fn new(
        id: impl Into<String>,
        owner: impl Into<String>,
        mime: impl Into<String>,
        bytes: Vec<u8>,
        ts: u64,
    ) -> Self {
        Self {
            id: id.into(),
            owner: owner.into(),
            mime: mime.into(),
            bytes,
            ts,
        }
    }
}

/// Serialize `Vec<u8>` as a base64 string over JSON (SurrealDB stores the `data` blob as a JSON
/// value; raw byte arrays are not a clean `serde_json::Value`). Transparent base64 keeps the
/// payload opaque and round-trips byte-identical (document-store scope: "byte-identical").
mod serde_bytes_base64 {
    use base64ct::{Base64, Encoding};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S, T>(bytes: &T, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: AsRef<[u8]> + ?Sized,
    {
        Base64::encode_string(bytes.as_ref()).serialize(ser)
    }

    pub fn deserialize<'de, D>(de: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(de)?;
        Base64::decode_vec(&s).map_err(serde::de::Error::custom)
    }
}
