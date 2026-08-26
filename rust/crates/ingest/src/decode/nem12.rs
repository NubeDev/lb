//! **NEM12** — AEMO's interval metering data format, the one an Australian electricity meter's data
//! actually arrives in (emailed monthly, as a `.csv` that is not a CSV in any useful sense).
//!
//! ### Why a named format lives in a core crate
//!
//! Rule 10 says the core knows no *extension*. A file format is not an extension: it is a way bytes
//! are shaped, like JSON or base64, and the platform already owns the data plane those bytes become.
//! The test that keeps this honest is the one this file passes — **nothing outside this file knows
//! NEM12 exists.** [`decode`](super::decode) resolves an opaque id through
//! [`FORMATS`](super::FORMATS); the mail source, the ingest verb, and the MCP surface all treat the
//! format as a string they were handed. Deleting this file removes a format; it does not change a
//! single caller.
//!
//! ### The record grammar (AEMO MDFF)
//!
//! ```text
//! 100,NEM12,<created YYYYMMDDhhmm>,<from participant>,<to participant>
//! 200,<NMI>,<config>,<register>,<suffix>,<stream>,<serial>,<UOM>,<interval minutes>,<next read>
//! 300,<YYYYMMDD>,<v1>,…,<vN>,<quality>,<reason>,<description>,<updated>,<msats loaded>
//! 400,<start interval>,<end interval>,<quality>,<reason>,<description>      (per-interval events)
//! 500,…                                                                     (B2B details)
//! 900
//! ```
//!
//! `N` is `1440 / interval minutes` — 96 for 15-minute data, 288 for 5-minute. Each `300` belongs to
//! the most recent `200`, which is what makes a file with several meters in it one file rather than
//! several.
//!
//! ### Three decisions worth stating
//!
//! **1. Values are period-ENDING.** Interval `i` of a day covers `[(i-1)·L, i·L)` and is stamped at
//! `i·L` — so the first 15-minute value of 2026-07-01 is timestamped `00:15`, and the last is
//! `00:00` the following day. This is the AEMO convention, and getting it backwards shifts every
//! meter in the estate by one interval — visible only as a subtly wrong peak-demand time, which is
//! the kind of error that survives for years.
//!
//! **2. The file carries no timezone and never will.** NEM12 times are NEM time (UTC+10, no DST) by
//! specification, but the *file* says so nowhere, and the same format is used by non-market meters
//! in local time. So the offset is [`DecodeOptions::offset_minutes`](super::DecodeOptions) — the
//! caller's configuration — and the decoder applies it without opinion.
//!
//! **3. A bad row is a warning, not a failed file.** A month of interval data with one unparseable
//! value must import the other 4,319 points. Only a file with no recognizable header at all fails.

use serde_json::{json, Map, Value};

use super::civil::{epoch_ms, parse_compact_date};
use super::{sample_at, DecodeError, DecodeInput, DecodeOptions, Decoded};

/// The registered format id. Opaque to every caller — see the module note.
pub const FORMAT: &str = "nem12";

/// Minutes in a day; `1440 / interval_minutes` is the expected value count of a `300` record.
const MINUTES_PER_DAY: u32 = 1440;

/// Does this look like a NEM12 file? The `100,NEM12` header record, allowing leading whitespace and
/// a UTF-8 BOM (both of which real exports carry).
pub fn looks_like(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(256)];
    let text = String::from_utf8_lossy(head);
    let text = text.trim_start_matches('\u{feff}').trim_start();
    let mut fields = text.split(',');
    matches!(
        (fields.next(), fields.next()),
        (Some("100"), Some(second)) if second.trim().eq_ignore_ascii_case("NEM12")
    )
}

/// The `200` record's context: everything a `300` under it needs to name and label its samples.
struct Block {
    nmi: String,
    suffix: String,
    uom: String,
    interval_minutes: u32,
    serial: String,
}

impl Block {
    /// `<nmi>.<suffix>` — the meter and the channel. Both are needed: one NMI commonly reports
    /// several suffixes (E1 consumption, B1 export, Q1 reactive) and collapsing them onto the NMI
    /// alone would interleave unrelated series into one.
    fn series(&self) -> String {
        if self.suffix.is_empty() {
            self.nmi.clone()
        } else {
            format!("{}.{}", self.nmi, self.suffix)
        }
    }

    fn expected_values(&self) -> usize {
        (MINUTES_PER_DAY / self.interval_minutes.max(1)) as usize
    }

    fn labels(&self) -> Map<String, Value> {
        let mut labels = Map::new();
        labels.insert("nmi".into(), json!(self.nmi));
        if !self.suffix.is_empty() {
            labels.insert("suffix".into(), json!(self.suffix));
        }
        if !self.uom.is_empty() {
            labels.insert("uom".into(), json!(self.uom));
        }
        if !self.serial.is_empty() {
            labels.insert("meterSerial".into(), json!(self.serial));
        }
        labels.insert("intervalMinutes".into(), json!(self.interval_minutes));
        labels
    }
}

/// Decode a NEM12 file into samples.
pub fn decode(input: &DecodeInput<'_>, options: &DecodeOptions) -> Result<Decoded, DecodeError> {
    if !looks_like(input.bytes) {
        return Err(DecodeError::malformed(
            FORMAT,
            "no `100,NEM12` header record — this is not a NEM12 file",
        ));
    }
    let text = String::from_utf8_lossy(input.bytes);
    let ceiling = options.sample_ceiling();

    let mut out = Decoded {
        format: FORMAT.into(),
        ..Default::default()
    };
    let mut block: Option<Block> = None;

    for (line_no, line) in text.lines().enumerate() {
        let line = line.trim_start_matches('\u{feff}').trim_end();
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(',').collect();
        match fields[0].trim() {
            "200" => match parse_block(&fields) {
                Ok(parsed) => block = Some(parsed),
                Err(reason) => {
                    // A broken 200 poisons every 300 that follows it, so the block is CLEARED
                    // rather than left pointing at the previous meter — silently attributing one
                    // meter's readings to another is the worst outcome available here.
                    block = None;
                    out.warnings
                        .push(format!("line {}: 200 record: {reason}", line_no + 1));
                }
            },
            "300" => {
                let Some(block) = block.as_ref() else {
                    out.warnings.push(format!(
                        "line {}: 300 record with no usable 200 record before it — skipped",
                        line_no + 1
                    ));
                    continue;
                };
                if out.samples.len() >= ceiling {
                    out.truncated = true;
                    break;
                }
                let series = options.series_name(&block.series());
                let labels = options.merge_labels(block.labels());
                match read_day(
                    &fields,
                    block,
                    &series,
                    &labels,
                    options.offset_minutes,
                    ceiling - out.samples.len(),
                ) {
                    Ok(day) => {
                        if !out.series.contains(&series) {
                            out.series.push(series);
                        }
                        out.truncated |= day.truncated;
                        out.warnings.extend(
                            day.warnings
                                .into_iter()
                                .map(|w| format!("line {}: {w}", line_no + 1)),
                        );
                        out.samples.extend(day.samples);
                    }
                    Err(reason) => out
                        .warnings
                        .push(format!("line {}: 300 record: {reason}", line_no + 1)),
                }
            }
            // 100 (header), 400 (interval events), 500 (B2B), 900 (end) carry no interval values.
            // 400 quality events are deliberately not modelled: they annotate intervals already
            // imported, and representing them would need a per-sample quality dimension the series
            // plane does not have. Named here so the omission is a decision, not an oversight.
            _ => {}
        }
        if out.truncated {
            break;
        }
    }

    if out.samples.is_empty() && out.warnings.is_empty() {
        out.warnings
            .push("no 300 interval records found in the file".into());
    }
    Ok(out)
}

/// Parse a `200` NMI-data-details record.
fn parse_block(fields: &[&str]) -> Result<Block, String> {
    // 200,NMI,config,register,suffix,stream,serial,UOM,interval,next-read
    let field = |i: usize| fields.get(i).map_or("", |f| f.trim());
    let nmi = field(1).to_string();
    if nmi.is_empty() {
        return Err("no NMI".into());
    }
    let interval_minutes: u32 = field(8)
        .parse()
        .map_err(|_| format!("interval length '{}' is not a number", field(8)))?;
    if interval_minutes == 0 || MINUTES_PER_DAY % interval_minutes != 0 {
        return Err(format!(
            "interval length {interval_minutes} does not divide a 1440-minute day"
        ));
    }
    Ok(Block {
        nmi,
        suffix: field(4).to_string(),
        uom: field(7).to_ascii_uppercase(),
        interval_minutes,
        serial: field(6).to_string(),
    })
}

/// One `300` record's samples.
struct Day {
    samples: Vec<crate::sample::Sample>,
    warnings: Vec<String>,
    truncated: bool,
}

fn read_day(
    fields: &[&str],
    block: &Block,
    series: &str,
    labels: &Value,
    offset_minutes: i64,
    headroom: usize,
) -> Result<Day, String> {
    let date_field = fields.get(1).map_or("", |f| f.trim());
    let (year, month, day_of_month) = parse_compact_date(date_field)
        .ok_or_else(|| format!("date '{date_field}' is not YYYYMMDD"))?;
    let expected = block.expected_values();
    if fields.len() < 2 + expected {
        return Err(format!(
            "expected {expected} interval values for a {}-minute day, found {}",
            block.interval_minutes,
            fields.len().saturating_sub(2)
        ));
    }
    // The quality method sits immediately after the values; it is a per-day annotation and rides on
    // every sample of that day so a consumer can tell an actual read (`A`) from a substitute (`S`)
    // or an estimate (`E`) without going back to the file.
    let quality = fields.get(2 + expected).map_or("", |f| f.trim());
    let labels = with_quality(labels, quality);

    let mut out = Day {
        samples: Vec::with_capacity(expected.min(headroom)),
        warnings: Vec::new(),
        truncated: false,
    };
    for index in 0..expected {
        if out.samples.len() >= headroom {
            out.truncated = true;
            break;
        }
        let raw = fields[2 + index].trim();
        if raw.is_empty() {
            // A blank interval is a real thing in NEM12 (a meter that reported nothing). Skipped
            // silently — writing a zero would invent consumption that did not happen.
            continue;
        }
        let Ok(value) = raw.parse::<f64>() else {
            out.warnings.push(format!(
                "interval {} value '{raw}' is not a number",
                index + 1
            ));
            continue;
        };
        if !value.is_finite() {
            out.warnings.push(format!(
                "interval {} value '{raw}' is not finite",
                index + 1
            ));
            continue;
        }
        // Period-ENDING: interval i covers [(i-1)·L, i·L) and is stamped at its end. See the
        // module note — reversing this shifts every reading by one interval.
        let minutes = (index as u32 + 1) * block.interval_minutes;
        let Some(ts_ms) = day_start_plus(year, month, day_of_month, minutes, offset_minutes) else {
            out.warnings.push(format!(
                "interval {} lands outside the representable range",
                index + 1
            ));
            continue;
        };
        out.samples.push(sample_at(
            series.to_string(),
            ts_ms,
            json!(value),
            labels.clone(),
        ));
    }
    Ok(out)
}

/// `date 00:00 + minutes`, in UTC, honouring the caller's fixed offset. Minutes may reach 1440
/// (the last interval of a day is midnight the next day), which rolls the date forward.
fn day_start_plus(
    year: i64,
    month: u32,
    day: u32,
    minutes: u32,
    offset_minutes: i64,
) -> Option<u64> {
    let midnight = epoch_ms(year, month, day, 0, 0, 0, offset_minutes)?;
    midnight.checked_add(minutes as u64 * 60_000)
}

/// Attach the `300` record's quality method to a labels object.
fn with_quality(labels: &Value, quality: &str) -> Value {
    if quality.is_empty() {
        return labels.clone();
    }
    let mut map = match labels {
        Value::Object(map) => map.clone(),
        _ => Map::new(),
    };
    map.insert("quality".into(), json!(quality));
    Value::Object(map)
}
