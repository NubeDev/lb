//! **`csv-grid`** — the generic shape: a timestamp column, then one column per series.
//!
//! ```text
//! timestamp,flow_temp,return_temp,pump_kw
//! 2026-07-01T00:15:00Z,71.2,58.4,3.1
//! 2026-07-01T00:30:00Z,71.4,58.1,3.0
//! ```
//!
//! This is what a data logger export, a spreadsheet "save as CSV", and most SCADA historians
//! actually produce, and it is the fallback [`detect`](super::detect) reaches for when a `.csv` is
//! not something more specific. One column becomes one series named by its header, so the file's own
//! vocabulary survives into the platform without a mapping table.
//!
//! ### What it deliberately does not do
//!
//! - **No column-mapping config.** A header is a series name. If a workspace wants different names
//!   it renames the series (`series.rename` exists) or fixes its export — encoding a per-file column
//!   map here would grow into a schema language, and the tall/long shape below already covers the
//!   cases a map is usually reached for.
//! - **No type inference beyond number/bool.** A non-numeric cell is a warning, not a string sample:
//!   the series plane's readers (bucketing, rollups, filters) are numeric, and quietly writing
//!   `"n/a"` as a payload produces a series that charts as a hole with no explanation.
//! - **No delimiter sniffing beyond tab vs comma.** Semicolon-separated "CSV" exists; it is a
//!   European spreadsheet locale artifact and the honest fix is at the export, not a heuristic that
//!   will one day split a legitimately comma-bearing field.
//!
//! Timestamps accept the three things real files contain: ISO 8601 (`2026-07-01T00:15:00Z` and the
//! space-separated variant), epoch seconds, and epoch milliseconds. Epoch values are distinguished
//! by magnitude — a 13-digit number is milliseconds — which is exact for every instant between 1973
//! and the year 33658.

use serde_json::{json, Map, Value};

use super::civil::epoch_ms;
use super::{sample_at, DecodeError, DecodeInput, DecodeOptions, Decoded};

/// The registered format id.
pub const FORMAT: &str = "csv-grid";

/// The magnitude at which a bare epoch number is read as milliseconds rather than seconds. Below
/// this, seconds; at or above, milliseconds. `1e11` seconds is the year 5138, and `1e11`
/// milliseconds is 1973 — so no real timestamp is ambiguous.
const MILLIS_THRESHOLD: i64 = 100_000_000_000;

pub fn decode(input: &DecodeInput<'_>, options: &DecodeOptions) -> Result<Decoded, DecodeError> {
    let text = String::from_utf8_lossy(input.bytes);
    let mut lines = text
        .lines()
        .map(|l| l.trim_start_matches('\u{feff}').trim_end())
        .filter(|l| !l.trim().is_empty());

    let header_line = lines
        .next()
        .ok_or_else(|| DecodeError::malformed(FORMAT, "the file is empty"))?;
    let delimiter = if header_line.contains('\t') {
        '\t'
    } else {
        ','
    };
    let headers: Vec<String> = header_line
        .split(delimiter)
        .map(|h| h.trim().trim_matches('"').to_string())
        .collect();
    if headers.len() < 2 {
        return Err(DecodeError::malformed(
            FORMAT,
            format!(
                "expected a timestamp column and at least one value column, found {} column(s)",
                headers.len()
            ),
        ));
    }

    let ceiling = options.sample_ceiling();
    let mut out = Decoded {
        format: FORMAT.into(),
        ..Default::default()
    };
    // Column index → its series name and labels, built once rather than per row.
    let columns: Vec<(String, Value)> = headers[1..]
        .iter()
        .map(|header| {
            let name = if header.is_empty() { "value" } else { header };
            let mut extra = Map::new();
            extra.insert("column".into(), json!(header));
            (options.series_name(name), options.merge_labels(extra))
        })
        .collect();

    for (row_no, line) in lines.enumerate() {
        if out.samples.len() >= ceiling {
            out.truncated = true;
            break;
        }
        let cells: Vec<&str> = line.split(delimiter).collect();
        let Some(ts_ms) = parse_timestamp(
            cells.first().map_or("", |c| c.trim()),
            options.offset_minutes,
        ) else {
            out.warnings.push(format!(
                "row {}: timestamp '{}' is not ISO 8601 or an epoch number",
                row_no + 2,
                cells.first().map_or("", |c| c.trim())
            ));
            continue;
        };
        for (index, (series, labels)) in columns.iter().enumerate() {
            if out.samples.len() >= ceiling {
                out.truncated = true;
                break;
            }
            let Some(raw) = cells.get(index + 1).map(|c| c.trim().trim_matches('"')) else {
                continue; // a short row: the remaining columns simply have no reading
            };
            if raw.is_empty() {
                continue;
            }
            let payload = match raw.parse::<f64>() {
                Ok(v) if v.is_finite() => json!(v),
                _ => match raw.to_ascii_lowercase().as_str() {
                    "true" => json!(true),
                    "false" => json!(false),
                    _ => {
                        out.warnings.push(format!(
                            "row {} column '{}': '{raw}' is not a number",
                            row_no + 2,
                            headers[index + 1]
                        ));
                        continue;
                    }
                },
            };
            if !out.series.contains(series) {
                out.series.push(series.clone());
            }
            out.samples
                .push(sample_at(series.clone(), ts_ms, payload, labels.clone()));
        }
    }

    if out.samples.is_empty() && out.warnings.is_empty() {
        out.warnings
            .push("no data rows found after the header".into());
    }
    Ok(out)
}

/// ISO 8601, epoch seconds, or epoch milliseconds → epoch milliseconds.
///
/// A trailing `Z` or an explicit `±hh:mm` offset in the string WINS over `offset_minutes`: the file
/// stated its zone, and a config default must not override a fact.
fn parse_timestamp(raw: &str, offset_minutes: i64) -> Option<u64> {
    let raw = raw.trim().trim_matches('"');
    if raw.is_empty() {
        return None;
    }
    // Epoch number.
    if raw.bytes().all(|b| b.is_ascii_digit()) {
        let n: i64 = raw.parse().ok()?;
        return u64::try_from(if n >= MILLIS_THRESHOLD { n } else { n * 1000 }).ok();
    }

    // ISO 8601 / `YYYY-MM-DD hh:mm[:ss]`, with an optional zone suffix.
    let (body, explicit_offset) = split_zone(raw);
    let (date, time) = body
        .split_once(['T', 't', ' '])
        .unwrap_or((body, "00:00:00"));
    let mut date_parts = date.split('-');
    let year: i64 = date_parts.next()?.parse().ok()?;
    let month: u32 = date_parts.next()?.parse().ok()?;
    let day: u32 = date_parts.next()?.parse().ok()?;
    // Drop any fractional seconds — the series plane's `ts` is milliseconds and the decoders'
    // `seq` derivation is per-second, so sub-second precision here would be a false promise.
    let time = time.split('.').next().unwrap_or(time);
    let mut time_parts = time.split(':');
    let hour: u32 = time_parts.next().unwrap_or("0").parse().ok()?;
    let minute: u32 = time_parts.next().unwrap_or("0").parse().ok()?;
    let second: u32 = time_parts.next().unwrap_or("0").parse().unwrap_or(0);
    epoch_ms(
        year,
        month,
        day,
        hour,
        minute,
        second,
        explicit_offset.unwrap_or(offset_minutes),
    )
}

/// Split a trailing zone designator off an ISO 8601 timestamp. Returns the body and the offset in
/// minutes when the string carried one.
fn split_zone(raw: &str) -> (&str, Option<i64>) {
    if let Some(body) = raw.strip_suffix(['Z', 'z']) {
        return (body, Some(0));
    }
    // `±hh:mm` / `±hhmm`, only after the time part (a leading `-` is a date separator, and the date
    // always has at least 8 characters before an offset could begin).
    let bytes = raw.as_bytes();
    for cut in (10..raw.len()).rev() {
        if bytes[cut] == b'+' || bytes[cut] == b'-' {
            let (body, zone) = raw.split_at(cut);
            let sign = if zone.starts_with('-') { -1 } else { 1 };
            let digits: String = zone[1..].chars().filter(char::is_ascii_digit).collect();
            if digits.len() != 4 {
                return (raw, None);
            }
            let hours: i64 = digits[0..2].parse().unwrap_or(0);
            let minutes: i64 = digits[2..4].parse().unwrap_or(0);
            return (body, Some(sign * (hours * 60 + minutes)));
        }
    }
    (raw, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_epoch_seconds_and_epoch_millis_all_land_on_the_same_instant() {
        let iso = parse_timestamp("2026-07-01T00:15:00Z", 0).unwrap();
        let secs = parse_timestamp("1782864900", 0).unwrap();
        let millis = parse_timestamp("1782864900000", 0).unwrap();
        assert_eq!(iso, secs);
        assert_eq!(iso, millis);
    }

    #[test]
    fn an_explicit_zone_in_the_file_beats_the_configured_offset() {
        // The file says +10:00; the config says UTC. The file wins.
        let stated = parse_timestamp("2026-07-01T00:15:00+10:00", 0).unwrap();
        let utc_midnight_quarter = parse_timestamp("2026-06-30T14:15:00Z", 0).unwrap();
        assert_eq!(stated, utc_midnight_quarter);
    }

    #[test]
    fn a_naive_timestamp_uses_the_configured_offset() {
        let naive = parse_timestamp("2026-07-01 00:15:00", 600).unwrap();
        assert_eq!(naive, parse_timestamp("2026-06-30T14:15:00Z", 0).unwrap());
    }
}
