//! The opaque keyset cursor for `series.read` paging — a bookmark, never a grant. It encodes only
//! the last-returned row's position on the unique sort key `(ts, seq, producer)` (the series is NOT
//! in the cursor: workspace and series always come from the token/request, so a replayed cursor under
//! another tenant's token seeks that tenant's namespace and resolves nothing).
//!
//! Versioned and base64-wrapped so the wire shape is opaque and a malformed/foreign cursor is
//! rejected cleanly (restart the chain) rather than mis-seeking.
//!
//! ## Why `ts` leads the key (v2, 2026-08-07)
//!
//! `seq` is assigned PER PRODUCER GENERATION and RESTARTS AT 0 when an extension restarts. Ordering a
//! page by `seq` therefore does not order it by time: after one restart the previous generation's
//! high-`seq` rows sort ABOVE the new generation's low-`seq` rows even though they are older. A
//! `direction:"back"` read then fills its page with the STALE generation and never reaches the recent
//! data, so every window wider than the newest generation returns the same wrong rows and the chart
//! draws a line that jumps backwards in time at the generation seam.
//!
//! Observed live 2026-08-07: 40/40 demo series returned unsorted rows, each with exactly one
//! ascending break at the `@1000000 → @2000001` seam. `ts` is the only key that is monotonic across
//! restarts, so it leads; `(seq, producer)` remain as tiebreakers to keep the key UNIQUE (two samples
//! may share a millisecond, and two producers may share a `seq`).
//!
//! Ordering by `ts` costs nothing: `series_ts_idx` on `(series, ts)` already exists (`schema.rs`), so
//! the seek stays O(page) rather than becoming an OFFSET scan.
//!
//! A `v1:` cursor is still DECODED (it carries no `ts`, so it seeks on the legacy key) — an in-flight
//! page chain from a node that predates this survives instead of erroring mid-scroll.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

/// A decoded keyset position: the `(ts, seq, producer)` of the last row a page returned.
///
/// `ts` is the primary key (see the module header — it is the only component monotonic across a
/// producer restart). `seq` and `producer` break the tie: two samples on one series may share a
/// millisecond, and two producers may share a `seq`, and seeking on a non-unique key would skip or
/// repeat rows at the tie.
///
/// `ts: None` is a legacy `v1:` cursor — decoded for compatibility, seeking on `(seq, producer)`
/// alone exactly as it did before.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    /// Epoch ms of the last-returned row. `None` only for a decoded `v1:` cursor.
    pub ts: Option<u64>,
    pub seq: u64,
    pub producer: String,
}

impl Cursor {
    /// Encode to the opaque wire form (`base64("v2:<ts>:<seq>:<producer>")`).
    ///
    /// A cursor with no `ts` (only reachable by re-encoding a decoded `v1:`) round-trips as `v1:` so
    /// it never claims a position on a key it does not carry.
    pub fn encode(&self) -> String {
        let s = match self.ts {
            Some(ts) => format!("v2:{}:{}:{}", ts, self.seq, self.producer),
            None => format!("v1:{}:{}", self.seq, self.producer),
        };
        URL_SAFE_NO_PAD.encode(s)
    }

    /// Decode a wire cursor. Any malformed, truncated, or unknown-version input is an error — the
    /// caller rejects the page cleanly; it never guesses a position.
    pub fn decode(wire: &str) -> Result<Cursor, String> {
        let raw = URL_SAFE_NO_PAD
            .decode(wire)
            .map_err(|_| "cursor: not base64".to_string())?;
        let s = String::from_utf8(raw).map_err(|_| "cursor: not utf8".to_string())?;
        if let Some(rest) = s.strip_prefix("v2:") {
            // `producer` may itself contain ':' (`ext:modbus/…@gen`), so split only the two numeric
            // leading fields and keep the remainder verbatim.
            let (ts, rest) = rest
                .split_once(':')
                .ok_or_else(|| "cursor: malformed".to_string())?;
            let (seq, producer) = rest
                .split_once(':')
                .ok_or_else(|| "cursor: malformed".to_string())?;
            return Ok(Cursor {
                ts: Some(
                    ts.parse::<u64>()
                        .map_err(|_| "cursor: bad ts".to_string())?,
                ),
                seq: seq
                    .parse::<u64>()
                    .map_err(|_| "cursor: bad seq".to_string())?,
                producer: producer.to_string(),
            });
        }
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
            ts: None,
            seq,
            producer: producer.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_round_trips_with_ts_leading() {
        let c = Cursor {
            ts: Some(1_786_096_195_989),
            seq: 283,
            producer: "ext:modbus/modbus.demo-sim-network@2000001".into(),
        };
        assert_eq!(Cursor::decode(&c.encode()).unwrap(), c);
    }

    /// `producer` contains ':' — the decoder must split only the two leading numeric fields.
    #[test]
    fn a_producer_containing_colons_survives_the_round_trip() {
        let c = Cursor {
            ts: Some(1),
            seq: 2,
            producer: "ext:modbus/modbus.net@3".into(),
        };
        let back = Cursor::decode(&c.encode()).unwrap();
        assert_eq!(back.producer, "ext:modbus/modbus.net@3");
        assert_eq!(back, c);
    }

    /// A cursor issued by a node that predates the `ts` key still decodes — an in-flight page chain
    /// survives the upgrade instead of erroring mid-scroll.
    #[test]
    fn a_v1_cursor_still_decodes_and_carries_no_ts() {
        let wire = URL_SAFE_NO_PAD.encode("v1:4144:ext:modbus/x@1000000");
        let c = Cursor::decode(&wire).unwrap();
        assert_eq!(c.ts, None);
        assert_eq!(c.seq, 4144);
        assert_eq!(c.producer, "ext:modbus/x@1000000");
        // …and re-encodes as v1, never claiming a `ts` position it does not have.
        assert_eq!(c.encode(), wire);
    }

    #[test]
    fn a_malformed_or_unknown_cursor_is_rejected_not_guessed() {
        assert!(Cursor::decode("!!!not base64!!!").is_err());
        assert!(Cursor::decode(&URL_SAFE_NO_PAD.encode("v9:1:2:p")).is_err());
        assert!(Cursor::decode(&URL_SAFE_NO_PAD.encode("v2:notanumber:2:p")).is_err());
        assert!(Cursor::decode(&URL_SAFE_NO_PAD.encode("v2:1")).is_err());
    }
}
