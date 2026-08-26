//! [`DecodeOptions`] — the caller's configuration for one decode.
//!
//! Everything here is a choice about **meaning** that the bytes themselves cannot settle, which is
//! the test for whether a knob belongs in this struct: if a decoder could work it out from the file,
//! it should, and there should be no option for it.
//!
//! - `series_prefix` — what namespace these series live in. The file knows its meter id; it does not
//!   know whether this workspace calls it `nem12.…` or `site-a.…`.
//! - `offset_minutes` — what the file's wall-clock times mean. NEM12 timestamps are NEM time
//!   (UTC+10, no DST) and say so nowhere in the file; a CSV exported from a spreadsheet is in
//!   whatever zone the exporter's laptop was in. Guessing is how a month of data lands an hour out.
//! - `labels` — the caller's own dimensions, attached to every sample the decode produces (the
//!   source it arrived from, the sender, the message it came in). The decoder adds the file's own
//!   dimensions on top; these are the *provenance*.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// How to interpret one file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecodeOptions {
    /// Prepended to every series name the decode produces (e.g. `"nem12."`). Empty ⇒ the decoder's
    /// own naming, unprefixed.
    #[serde(default)]
    pub series_prefix: String,
    /// How far ahead of UTC the file's wall-clock timestamps are, in minutes (`600` = AEST).
    /// Ignored by formats whose timestamps are already absolute.
    #[serde(default)]
    pub offset_minutes: i64,
    /// Caller-supplied provenance labels, merged into every sample's `labels`.
    #[serde(default)]
    pub labels: Value,
    /// The upper bound on samples one decode may produce. A file is untrusted input — a malformed
    /// or hostile 5 MB CSV can describe tens of millions of points, and a decoder that returned
    /// them all would exhaust the node's memory before the ingest bound ever saw a sample. Zero ⇒
    /// [`DEFAULT_MAX_SAMPLES`].
    #[serde(default)]
    pub max_samples: usize,
}

/// The default per-file sample ceiling. Generous enough for a year of 5-minute data on 20 channels
/// (≈2.1 M), small enough that hitting it is a signal rather than an outage.
pub const DEFAULT_MAX_SAMPLES: usize = 2_500_000;

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            series_prefix: String::new(),
            offset_minutes: 0,
            labels: Value::Null,
            max_samples: DEFAULT_MAX_SAMPLES,
        }
    }
}

impl DecodeOptions {
    /// The effective sample ceiling.
    pub fn sample_ceiling(&self) -> usize {
        if self.max_samples == 0 {
            DEFAULT_MAX_SAMPLES
        } else {
            self.max_samples
        }
    }

    /// `self.labels` merged with the decoder's own `extra` dimensions. The decoder's win on a key
    /// collision: they describe what the data *is* (unit, meter, channel), the caller's describe
    /// where it *came from*, and a caller must not be able to relabel a meter's unit by
    /// misconfiguring a mail source.
    pub fn merge_labels(&self, extra: Map<String, Value>) -> Value {
        let mut merged = match &self.labels {
            Value::Object(map) => map.clone(),
            _ => Map::new(),
        };
        merged.extend(extra);
        if merged.is_empty() {
            Value::Null
        } else {
            Value::Object(merged)
        }
    }

    /// The full series name for a decoder-chosen `name`.
    pub fn series_name(&self, name: &str) -> String {
        format!("{}{}", self.series_prefix, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_decoders_own_labels_win_a_collision() {
        let opts = DecodeOptions {
            labels: json!({"uom": "LIES", "source": "mail"}),
            ..Default::default()
        };
        let mut extra = Map::new();
        extra.insert("uom".into(), json!("KWH"));
        let merged = opts.merge_labels(extra);
        assert_eq!(
            merged["uom"],
            json!("KWH"),
            "a caller cannot relabel a meter's unit"
        );
        assert_eq!(merged["source"], json!("mail"), "provenance survives");
    }

    #[test]
    fn a_zero_ceiling_means_the_default_not_zero_samples() {
        let opts = DecodeOptions {
            max_samples: 0,
            ..Default::default()
        };
        assert_eq!(opts.sample_ceiling(), DEFAULT_MAX_SAMPLES);
    }
}
