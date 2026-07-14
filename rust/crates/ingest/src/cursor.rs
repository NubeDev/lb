//! The opaque keyset cursor for `series.read` paging — a bookmark, never a grant. It encodes only
//! the last-returned row's position on the unique sort key `(seq, producer)` (the series is NOT in
//! the cursor: workspace and series always come from the token/request, so a replayed cursor under
//! another tenant's token seeks that tenant's namespace and resolves nothing).
//!
//! Versioned (`v1:`) and base64-wrapped so the wire shape is opaque and a malformed/foreign cursor
//! is rejected cleanly (restart the chain) rather than mis-seeking.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

/// A decoded keyset position: the `(seq, producer)` of the last row a page returned. `producer`
/// breaks the tie when two producers share a `seq` on one series — seeking on `seq` alone would
/// skip or repeat rows at the tie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    pub seq: u64,
    pub producer: String,
}

impl Cursor {
    /// Encode to the opaque wire form (`base64("v1:<seq>:<producer>")`).
    pub fn encode(&self) -> String {
        URL_SAFE_NO_PAD.encode(format!("v1:{}:{}", self.seq, self.producer))
    }

    /// Decode a wire cursor. Any malformed, truncated, or unknown-version input is an error — the
    /// caller rejects the page cleanly; it never guesses a position.
    pub fn decode(wire: &str) -> Result<Cursor, String> {
        let raw = URL_SAFE_NO_PAD
            .decode(wire)
            .map_err(|_| "cursor: not base64".to_string())?;
        let s = String::from_utf8(raw).map_err(|_| "cursor: not utf8".to_string())?;
        let rest = s
            .strip_prefix("v1:")
            .ok_or_else(|| "cursor: unknown version".to_string())?;
        let (seq, producer) = rest
            .split_once(':')
            .ok_or_else(|| "cursor: malformed".to_string())?;
        let seq = seq
            .parse::<u64>()
            .map_err(|_| "cursor: bad seq".to_string())?;
        Ok(Cursor {
            seq,
            producer: producer.to_string(),
        })
    }
}
