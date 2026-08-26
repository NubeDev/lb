//! **Files → samples.** The seam that lets an arbitrary attached file become series data
//! (mail-source scope, "attachment → ingest").
//!
//! A `Sample` is the platform's one data-plane envelope, and everything that produces one today does
//! so by *speaking JSON at `ingest.write`*. A file does not. A meter export, a logger dump, a
//! spreadsheet emailed once a month — these are the way a very large amount of real-world data
//! actually arrives, and until now the platform had no answer for them beyond "write a bespoke
//! parser somewhere".
//!
//! **This is a format registry, not a product feature.** Two rules keep it that way:
//!
//! 1. **The format id is opaque data.** [`decode`] takes a `&str` and looks it up; nothing branches
//!    on a *caller* (rule 10's shape, one layer down). A new format is a new file in this folder and
//!    one row in [`FORMATS`] — it is not a change to the mail source, the ingest verb, or anything
//!    that calls this.
//! 2. **A decoder is a pure function of bytes + options.** No store, no clock, no network, no
//!    workspace. Every decoder in this folder is exercisable from a byte literal, which is why the
//!    tests can use the real files this shipped for.
//!
//! ### The `seq` decision, which is load-bearing
//!
//! `seq` is half of ingest's dedup identity `(series, producer, seq)`, and a decoder must choose it.
//! The obvious choice — "0, 1, 2, … in file order" — is wrong in a way that only shows up in
//! production: the *same* file re-imported is fine, but a **second** file covering an overlapping
//! period (a corrected re-issue, a monthly export that repeats the last week) would re-use seq 0..N
//! for *different* instants and silently overwrite real data.
//!
//! So every decoder here derives `seq` from the sample's own timestamp: **`seq = ts_ms / 1000`**
//! (epoch seconds). That makes the identity a property of the *instant*, not of the file, so:
//! re-importing anything is an exact idempotent upsert; overlapping files converge instead of
//! colliding; and `series.latest` (highest seq) still means "newest", because seq now rises with
//! time. Sub-second data would collide — no format here produces it, and one that did must pick a
//! finer derivation rather than falling back to file order.

mod civil;
mod csv_grid;
mod error;
mod nem12;
mod options;

pub use error::DecodeError;
pub use options::{DecodeOptions, DEFAULT_MAX_SAMPLES as DEFAULT_MAX_DECODE_SAMPLES};

use crate::sample::Sample;

/// The bytes to decode, plus the two hints about what they are.
#[derive(Debug, Clone, Copy)]
pub struct DecodeInput<'a> {
    /// The sender-declared filename. **Untrusted**, and used only as a hint and a label.
    pub filename: &'a str,
    /// The declared content type. Frequently `application/octet-stream` and therefore worth less
    /// than the extension.
    pub mime: &'a str,
    pub bytes: &'a [u8],
}

impl<'a> DecodeInput<'a> {
    pub fn new(filename: &'a str, mime: &'a str, bytes: &'a [u8]) -> Self {
        Self {
            filename,
            mime,
            bytes,
        }
    }

    /// The lower-cased extension without the dot, or `""`.
    pub fn extension(&self) -> String {
        self.filename
            .rsplit_once('.')
            .map_or(String::new(), |(_, ext)| ext.trim().to_ascii_lowercase())
    }
}

/// What one decode produced.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Decoded {
    /// The format that ran (the resolved id, so a caller that passed [`AUTO`] learns what it got).
    pub format: String,
    /// The samples, with `producer` left **empty** — the ingest write path stamps the authenticated
    /// principal onto it, and a decoder inventing one would be forging a producer identity.
    pub samples: Vec<Sample>,
    /// The distinct series produced, sorted. The caller's summary line ("3 series, 288 samples")
    /// and the inbox item's meta come from here.
    pub series: Vec<String>,
    /// Rows that could not be read. See [`DecodeError`]'s note: these accompany a *successful*
    /// decode, because failing a month of data over one bad cell is the worse outcome.
    pub warnings: Vec<String>,
    /// The sample ceiling was hit and the decode stopped early. Never silent — a truncated import
    /// that reported success is the "looks like it worked" failure this crate avoids elsewhere too.
    pub truncated: bool,
}

/// The format id meaning "work it out from the bytes". Resolved by [`detect`] before dispatch.
pub const AUTO: &str = "auto";

/// One registered format: its id and a one-line description for the `formats` listing.
pub struct FormatInfo {
    pub id: &'static str,
    pub description: &'static str,
}

/// Every format this node can decode. The **only** place a format id becomes a code path.
pub const FORMATS: &[FormatInfo] = &[
    FormatInfo {
        id: nem12::FORMAT,
        description: "AEMO NEM12 interval meter data (100/200/300 records)",
    },
    FormatInfo {
        id: csv_grid::FORMAT,
        description: "CSV with a leading timestamp column and one series per remaining column",
    },
];

/// Decode `input` as `format`, or as whatever [`detect`] identifies when `format` is [`AUTO`].
///
/// Returns `Err` only when *nothing* could be read (see [`DecodeError`]); a partially-readable file
/// succeeds with `warnings`.
pub fn decode(
    format: &str,
    input: &DecodeInput<'_>,
    options: &DecodeOptions,
) -> Result<Decoded, DecodeError> {
    let resolved = if format.trim().is_empty() || format.eq_ignore_ascii_case(AUTO) {
        detect(input).ok_or_else(|| {
            DecodeError::malformed(
                AUTO,
                format!(
                    "cannot identify '{}' ({}) as any known format — name one of: {}",
                    input.filename,
                    input.mime,
                    FORMATS.iter().map(|f| f.id).collect::<Vec<_>>().join(", ")
                ),
            )
        })?
    } else {
        FORMATS
            .iter()
            .find(|f| f.id.eq_ignore_ascii_case(format.trim()))
            .map(|f| f.id)
            .ok_or_else(|| DecodeError::UnknownFormat(format.trim().to_string()))?
    };

    let mut decoded = match resolved {
        nem12::FORMAT => nem12::decode(input, options),
        csv_grid::FORMAT => csv_grid::decode(input, options),
        // Unreachable while `FORMATS` and this match agree; a `FormatInfo` added without an arm
        // lands here rather than silently decoding as something else.
        other => Err(DecodeError::UnknownFormat(other.to_string())),
    }?;
    decoded.format = resolved.to_string();
    decoded.series.sort();
    decoded.series.dedup();
    Ok(decoded)
}

/// Identify a file's format from its bytes and its name, or `None`.
///
/// **Content beats name.** A NEM12 file arrives named `.csv`, `.dat`, `ZZZZ035361_nem12#…#TCAUSTM`,
/// or with no extension at all, so the header record is checked first; the extension is only the
/// fallback that distinguishes a generic CSV from an unknown blob.
pub fn detect(input: &DecodeInput<'_>) -> Option<&'static str> {
    if nem12::looks_like(input.bytes) {
        return Some(nem12::FORMAT);
    }
    let ext = input.extension();
    let mime = input.mime.to_ascii_lowercase();
    if ext == "csv" || ext == "tsv" || mime == "text/csv" {
        return Some(csv_grid::FORMAT);
    }
    None
}

/// The dedup `seq` for a sample at `ts_ms` — epoch seconds. See the module note; this is the one
/// place the derivation lives so two decoders cannot disagree about it.
pub(crate) fn seq_for(ts_ms: u64) -> u64 {
    ts_ms / 1000
}

/// Build a sample with the producer left empty (stamped by `ingest.write`) and the seq derived from
/// the timestamp.
pub(crate) fn sample_at(
    series: String,
    ts_ms: u64,
    payload: serde_json::Value,
    labels: serde_json::Value,
) -> Sample {
    Sample {
        series,
        producer: String::new(),
        ts: ts_ms,
        seq: seq_for(ts_ms),
        payload,
        labels,
        qos: crate::sample::Qos::MustDeliver,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_format_id_is_named_not_guessed() {
        let input = DecodeInput::new("a.csv", "text/csv", b"ts,v\n1,2\n");
        let err = decode("parquet", &input, &DecodeOptions::default()).unwrap_err();
        assert_eq!(err, DecodeError::UnknownFormat("parquet".into()));
    }

    #[test]
    fn auto_on_an_unidentifiable_blob_says_what_it_could_have_been() {
        let input = DecodeInput::new("mystery.bin", "application/octet-stream", b"\x00\x01\x02");
        let err = decode(AUTO, &input, &DecodeOptions::default()).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("nem12"), "{message}");
        assert!(message.contains("csv"), "{message}");
    }

    #[test]
    fn every_registered_format_has_a_dispatch_arm() {
        // A `FormatInfo` added without a match arm above would fall through to `UnknownFormat`.
        for info in FORMATS {
            let input = DecodeInput::new("x", "", b"");
            let err = decode(info.id, &input, &DecodeOptions::default()).unwrap_err();
            assert!(
                !matches!(err, DecodeError::UnknownFormat(_)),
                "format '{}' is registered but has no dispatch arm",
                info.id
            );
        }
    }

    #[test]
    fn seq_rises_with_time_so_latest_still_means_newest() {
        assert!(seq_for(1_787_609_400_000) > seq_for(1_787_609_000_000));
    }
}
