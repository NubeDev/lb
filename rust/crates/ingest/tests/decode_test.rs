//! The file decoders against **real files**, including the actual four-channel NEM12 export this
//! slice was built for (`fixtures/nem12-4-channel.csv`, 163 KB, 55 days, four channels).
//!
//! Using the genuine file rather than a hand-written snippet is the point: the snippet you invent
//! never has the blank `RegisterID`, the mid-file `400` quality-event records, the `V` quality
//! method, or the trailing comma on the `200` record — and those are exactly the things a parser
//! written from the spec gets wrong.

use lb_ingest::{decode, detect, DecodeInput, DecodeOptions, AUTO};

/// The real export. Four channels (B1/E1 in kWh, K1/Q1 in kVArh), 15-minute intervals,
/// 2026-07-01 → 2026-08-24 inclusive.
const REAL_NEM12: &[u8] = include_bytes!("fixtures/nem12-4-channel.csv");

/// AEST — NEM time. The file says this nowhere, which is why it is configuration.
const NEM_OFFSET: i64 = 600;

fn nem_options() -> DecodeOptions {
    DecodeOptions {
        series_prefix: "nem12.".into(),
        offset_minutes: NEM_OFFSET,
        ..Default::default()
    }
}

#[test]
fn the_real_export_is_detected_without_being_told_what_it_is() {
    // Named `.csv`, and it is not a CSV. Content beats name.
    let input = DecodeInput::new(
        "ZZZZ035361_nem12#0045575584#TCAUSTM.csv",
        "text/csv",
        REAL_NEM12,
    );
    assert_eq!(detect(&input), Some("nem12"));
}

#[test]
fn the_real_export_yields_one_series_per_channel() {
    let input = DecodeInput::new("meter.csv", "text/csv", REAL_NEM12);
    let out = decode(AUTO, &input, &nem_options()).expect("decode");

    assert_eq!(out.format, "nem12");
    assert_eq!(
        out.series,
        vec![
            "nem12.ZZZZ035361.B1",
            "nem12.ZZZZ035361.E1",
            "nem12.ZZZZ035361.K1",
            "nem12.ZZZZ035361.Q1",
        ],
        "one NMI reporting four channels must not collapse onto one series"
    );
    // 220 `300` records × 96 fifteen-minute intervals, none blank in this file.
    assert_eq!(out.samples.len(), 220 * 96);
    assert!(!out.truncated);
    assert!(
        out.warnings.is_empty(),
        "a real, valid export must decode clean: {:?}",
        out.warnings
    );
}

#[test]
fn the_first_interval_is_period_ending_in_nem_time() {
    let input = DecodeInput::new("meter.csv", "text/csv", REAL_NEM12);
    let out = decode("nem12", &input, &nem_options()).expect("decode");

    let first = out
        .samples
        .iter()
        .find(|s| s.series == "nem12.ZZZZ035361.B1")
        .expect("B1 samples");
    // The file's first 300 record is `300,20260701,…`. Interval 1 covers 00:00–00:15 and is stamped
    // at its END: 2026-07-01T00:15+10:00 == 2026-06-30T14:15:00Z == 1782828900s.
    assert_eq!(
        first.ts, 1_782_828_900_000,
        "period-ENDING in NEM time; a period-starting reading would be 900_000 ms earlier"
    );
    assert_eq!(
        first.seq,
        first.ts / 1000,
        "seq is derived from the instant"
    );
}

#[test]
fn the_last_interval_of_a_day_is_midnight_the_next_day() {
    let input = DecodeInput::new("meter.csv", "text/csv", REAL_NEM12);
    let out = decode("nem12", &input, &nem_options()).expect("decode");

    let b1: Vec<u64> = out
        .samples
        .iter()
        .filter(|s| s.series == "nem12.ZZZZ035361.B1")
        .map(|s| s.ts)
        .collect();
    // Interval 96 of 2026-07-01 is stamped 2026-07-02T00:00+10:00 — 24h after interval 96 would be
    // if it were period-starting. Consecutive samples are exactly 15 minutes apart with no gap at
    // the day boundary, which is the property that proves the roll-forward works.
    for pair in b1.windows(2) {
        assert_eq!(
            pair[1] - pair[0],
            15 * 60 * 1000,
            "a 15-minute grid must have no seam at the day boundary"
        );
    }
    assert_eq!(b1.len(), 55 * 96, "55 days of 15-minute data");
}

#[test]
fn every_sample_carries_the_meters_own_dimensions() {
    let input = DecodeInput::new("meter.csv", "text/csv", REAL_NEM12);
    let out = decode("nem12", &input, &nem_options()).expect("decode");

    let k1 = out
        .samples
        .iter()
        .find(|s| s.series == "nem12.ZZZZ035361.K1")
        .expect("K1 samples");
    assert_eq!(k1.labels["nmi"], "ZZZZ035361");
    assert_eq!(k1.labels["suffix"], "K1");
    assert_eq!(k1.labels["uom"], "KVARH", "K1 is reactive energy, not kWh");
    assert_eq!(k1.labels["meterSerial"], "023386");
    assert_eq!(k1.labels["intervalMinutes"], 15);
    // The `300` record's quality method rides along, so a consumer can tell an actual read from a
    // substituted or variable one without re-reading the file.
    assert!(
        k1.labels.get("quality").is_some(),
        "the quality method must survive: {:?}",
        k1.labels
    );
    let b1 = out
        .samples
        .iter()
        .find(|s| s.series == "nem12.ZZZZ035361.B1")
        .expect("B1");
    assert_eq!(b1.labels["uom"], "KWH");
}

#[test]
fn caller_provenance_labels_ride_along_but_cannot_relabel_the_meter() {
    let input = DecodeInput::new("meter.csv", "text/csv", REAL_NEM12);
    let options = DecodeOptions {
        labels: serde_json::json!({"source": "mail:meter-data", "uom": "NONSENSE"}),
        ..nem_options()
    };
    let out = decode("nem12", &input, &options).expect("decode");
    let sample = &out.samples[0];
    assert_eq!(sample.labels["source"], "mail:meter-data");
    assert_eq!(
        sample.labels["uom"], "KWH",
        "the file's unit is not overridable by config"
    );
}

/// The idempotency the whole import path leans on: the same bytes decoded twice produce the same
/// `(series, seq)` for every sample, so re-importing an email is an exact upsert rather than a
/// duplicate. Re-derive from the instant, never from file order.
#[test]
fn decoding_the_same_file_twice_produces_identical_dedup_keys() {
    let input = DecodeInput::new("meter.csv", "text/csv", REAL_NEM12);
    let first = decode("nem12", &input, &nem_options()).expect("decode");
    let second = decode("nem12", &input, &nem_options()).expect("decode");

    let keys = |d: &lb_ingest::Decoded| -> Vec<(String, u64)> {
        d.samples
            .iter()
            .map(|s| (s.series.clone(), s.seq))
            .collect()
    };
    assert_eq!(keys(&first), keys(&second));
    // …and distinct within a single decode, or the file would overwrite itself.
    let mut sorted = keys(&first);
    let total = sorted.len();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        total,
        "a decode must not emit two samples at one dedup key"
    );
}

/// A second file covering an OVERLAPPING period must converge on the same rows, not collide with
/// them. This is the failure that file-order `seq` would have shipped with, and it would only have
/// appeared when a supplier re-issued a corrected month.
#[test]
fn an_overlapping_re_issue_lands_on_the_same_dedup_keys() {
    let full = decode(
        "nem12",
        &DecodeInput::new("meter.csv", "text/csv", REAL_NEM12),
        &nem_options(),
    )
    .expect("decode");

    // A re-issue containing only the LAST day of the same meter/channel.
    let last_day = format!(
        "100,NEM12,202608250810,TCAUSTM\r\n200,ZZZZ035361,B1E1K1Q1,,B1,,023386,KWH,15,\r\n{}\r\n900\r\n",
        std::str::from_utf8(REAL_NEM12)
            .expect("utf-8")
            .lines()
            .filter(|l| l.starts_with("300,20260824,"))
            .next()
            .expect("the 2026-08-24 row")
    );
    let reissue = decode(
        "nem12",
        &DecodeInput::new("reissue.csv", "text/csv", last_day.as_bytes()),
        &nem_options(),
    )
    .expect("decode");

    assert_eq!(reissue.samples.len(), 96);
    let full_keys: std::collections::HashSet<(String, u64)> = full
        .samples
        .iter()
        .map(|s| (s.series.clone(), s.seq))
        .collect();
    for sample in &reissue.samples {
        assert!(
            full_keys.contains(&(sample.series.clone(), sample.seq)),
            "a re-issued interval must upsert the original row, not create a second one: {} seq {}",
            sample.series,
            sample.seq
        );
    }
}

#[test]
fn a_broken_row_is_a_warning_and_the_rest_of_the_file_still_imports() {
    let file = "100,NEM12,202608250810,TCAUSTM\n\
                200,NMI1,C,,E1,,SER,KWH,1440,\n\
                300,20260701,1.5,A,,,20260702014509,\n\
                300,BADDATE,2.5,A,,,20260702014509,\n\
                300,20260703,notanumber,A,,,20260704014509,\n\
                900\n";
    let out = decode(
        "nem12",
        &DecodeInput::new("x.csv", "text/csv", file.as_bytes()),
        &DecodeOptions::default(),
    )
    .expect("a file with bad rows still decodes");

    assert_eq!(out.samples.len(), 1, "the one good row must survive");
    assert_eq!(out.samples[0].payload, serde_json::json!(1.5));
    assert_eq!(out.warnings.len(), 2, "{:?}", out.warnings);
    assert!(
        out.warnings.iter().any(|w| w.contains("BADDATE")),
        "{:?}",
        out.warnings
    );
    assert!(
        out.warnings.iter().any(|w| w.contains("notanumber")),
        "{:?}",
        out.warnings
    );
}

/// A `300` under a broken `200` must be dropped, not attributed to the previous meter — the silent
/// data-corruption failure this decoder refuses to have.
#[test]
fn rows_under_a_broken_header_are_never_attributed_to_the_previous_meter() {
    let file = "100,NEM12,202608250810,TCAUSTM\n\
                200,GOODNMI,C,,E1,,SER,KWH,1440,\n\
                300,20260701,1.5,A,,,20260702014509,\n\
                200,BROKEN,C,,E1,,SER,KWH,notanumber,\n\
                300,20260702,9.9,A,,,20260703014509,\n\
                900\n";
    let out = decode(
        "nem12",
        &DecodeInput::new("x.csv", "text/csv", file.as_bytes()),
        &DecodeOptions::default(),
    )
    .expect("decode");

    assert_eq!(out.samples.len(), 1);
    assert_eq!(out.samples[0].series, "GOODNMI.E1");
    assert_eq!(
        out.samples[0].payload,
        serde_json::json!(1.5),
        "9.9 belongs to no meter and must not have landed on GOODNMI"
    );
}

#[test]
fn the_sample_ceiling_truncates_loudly_rather_than_exhausting_memory() {
    let out = decode(
        "nem12",
        &DecodeInput::new("meter.csv", "text/csv", REAL_NEM12),
        &DecodeOptions {
            max_samples: 100,
            ..nem_options()
        },
    )
    .expect("decode");
    assert_eq!(out.samples.len(), 100);
    assert!(
        out.truncated,
        "a truncated import that reported success is the worst outcome"
    );
}

#[test]
fn a_plain_csv_grid_becomes_one_series_per_column() {
    let file = "timestamp,flow_temp,return_temp,pump_on\n\
                2026-07-01T00:15:00Z,71.2,58.4,true\n\
                2026-07-01T00:30:00Z,71.4,,false\n\
                2026-07-01T00:45:00Z,oops,58.0,true\n";
    let out = decode(
        AUTO,
        &DecodeInput::new("plant.csv", "text/csv", file.as_bytes()),
        &DecodeOptions {
            series_prefix: "plant.".into(),
            ..Default::default()
        },
    )
    .expect("decode");

    assert_eq!(out.format, "csv-grid");
    assert_eq!(
        out.series,
        vec!["plant.flow_temp", "plant.pump_on", "plant.return_temp"]
    );
    // 3 rows × 3 columns, minus the blank return_temp and the unparseable flow_temp.
    assert_eq!(out.samples.len(), 7);
    assert_eq!(out.warnings.len(), 1, "{:?}", out.warnings);
    let pump = out
        .samples
        .iter()
        .find(|s| s.series == "plant.pump_on")
        .expect("pump_on");
    assert_eq!(
        pump.payload,
        serde_json::json!(true),
        "a boolean column stays boolean"
    );
}

#[test]
fn a_csv_with_no_value_column_is_a_malformed_file_not_an_empty_import() {
    let err = decode(
        "csv-grid",
        &DecodeInput::new("one.csv", "text/csv", b"timestamp\n2026-07-01T00:00:00Z\n"),
        &DecodeOptions::default(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("value column"), "{err}");
}
