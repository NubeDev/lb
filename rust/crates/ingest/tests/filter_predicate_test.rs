//! The write-time normalize predicates, in isolation (series-normalize scope). Pure decisions: no
//! store, no batch, no transaction — just "given this filter, this payload, and this anchor, does
//! the sample land?". The store-backed half is `series_filter_test.rs`.

use lb_ingest::{decide, Deadband, Decision, Filter, LastCommitted, Range, RangeMode, Reason};
use serde_json::json;

fn anchor(ts: u64, value: f64) -> LastCommitted {
    LastCommitted {
        ts,
        value: Some(value),
    }
}

#[test]
fn drop_mutes_everything_including_non_numerics() {
    let f = Filter {
        drop: true,
        ..Default::default()
    };
    assert_eq!(
        decide(&f, &json!(1.0), 10, None),
        Decision::Drop(Reason::Muted)
    );
    assert_eq!(
        decide(&f, &json!("open"), 10, None),
        Decision::Drop(Reason::Muted)
    );
}

#[test]
fn range_drops_out_of_band_and_clamp_returns_the_bound() {
    let dropping = Filter {
        range: Some(Range {
            min: Some(-40.0),
            max: Some(120.0),
            mode: RangeMode::Drop,
        }),
        ..Default::default()
    };
    assert_eq!(
        decide(&dropping, &json!(-9999.0), 1, None),
        Decision::Drop(Reason::Range)
    );
    assert_eq!(decide(&dropping, &json!(21.5), 1, None), Decision::Keep);

    let clamping = Filter {
        range: Some(Range {
            mode: RangeMode::Clamp,
            ..dropping.range.unwrap()
        }),
        ..Default::default()
    };
    assert_eq!(
        decide(&clamping, &json!(-9999.0), 1, None),
        Decision::Clamp(-40.0)
    );
    assert_eq!(
        decide(&clamping, &json!(500.0), 1, None),
        Decision::Clamp(120.0)
    );
    // In band → stored as read, NOT reported as a clamp.
    assert_eq!(decide(&clamping, &json!(21.5), 1, None), Decision::Keep);
}

#[test]
fn a_non_numeric_payload_skips_the_numeric_predicates() {
    let f = Filter {
        range: Some(Range {
            min: Some(0.0),
            max: Some(1.0),
            mode: RangeMode::Drop,
        }),
        deadband: Some(Deadband {
            abs: Some(1000.0),
            pct: None,
        }),
        ..Default::default()
    };
    // A string payload is out of ANY numeric band and infinitely "unchanged" — it must still land.
    assert_eq!(
        decide(&f, &json!("door-open"), 5, Some(&anchor(1, 0.0))),
        Decision::Keep
    );
    assert_eq!(
        decide(&f, &json!({"state": "on"}), 5, Some(&anchor(1, 0.0))),
        Decision::Keep
    );
}

#[test]
fn min_interval_keeps_the_first_sample_of_each_interval() {
    let f = Filter {
        min_interval_ms: 1_000,
        ..Default::default()
    };
    // Nothing committed yet → the first sample always lands.
    assert_eq!(decide(&f, &json!(1.0), 5_000, None), Decision::Keep);
    let a = anchor(5_000, 1.0);
    assert_eq!(
        decide(&f, &json!(2.0), 5_400, Some(&a)),
        Decision::Drop(Reason::MinInterval)
    );
    assert_eq!(
        decide(&f, &json!(2.0), 5_999, Some(&a)),
        Decision::Drop(Reason::MinInterval)
    );
    // Exactly one interval later is the next FIRST.
    assert_eq!(decide(&f, &json!(2.0), 6_000, Some(&a)), Decision::Keep);
}

#[test]
fn deadband_abs_and_pct_and_the_abs_precedence() {
    let abs = Filter {
        deadband: Some(Deadband {
            abs: Some(0.5),
            pct: None,
        }),
        ..Default::default()
    };
    let a = anchor(1, 20.0);
    assert_eq!(
        decide(&abs, &json!(20.4), 2, Some(&a)),
        Decision::Drop(Reason::Deadband)
    );
    assert_eq!(decide(&abs, &json!(20.5), 2, Some(&a)), Decision::Keep);
    assert_eq!(decide(&abs, &json!(19.5), 2, Some(&a)), Decision::Keep);

    let pct = Filter {
        deadband: Some(Deadband {
            abs: None,
            pct: Some(10.0),
        }),
        ..Default::default()
    };
    // 10% of 20.0 = 2.0
    assert_eq!(
        decide(&pct, &json!(21.9), 2, Some(&a)),
        Decision::Drop(Reason::Deadband)
    );
    assert_eq!(decide(&pct, &json!(22.0), 2, Some(&a)), Decision::Keep);

    // Both set → `abs` wins (0.5), so a move of 1.0 that `pct` would have suppressed lands.
    let both = Filter {
        deadband: Some(Deadband {
            abs: Some(0.5),
            pct: Some(10.0),
        }),
        ..Default::default()
    };
    assert_eq!(decide(&both, &json!(21.0), 2, Some(&a)), Decision::Keep);
}

#[test]
fn order_is_drop_then_range_then_min_interval_then_deadband() {
    // A sample violating ALL of them reports the CHEAPEST reason — the documented order.
    let all = Filter {
        drop: true,
        min_interval_ms: 10_000,
        deadband: Some(Deadband {
            abs: Some(100.0),
            pct: None,
        }),
        range: Some(Range {
            min: Some(0.0),
            max: Some(1.0),
            mode: RangeMode::Drop,
        }),
    };
    let a = anchor(1_000, 0.5);
    assert_eq!(
        decide(&all, &json!(-5.0), 1_001, Some(&a)),
        Decision::Drop(Reason::Muted)
    );

    let no_mute = Filter { drop: false, ..all };
    assert_eq!(
        decide(&no_mute, &json!(-5.0), 1_001, Some(&a)),
        Decision::Drop(Reason::Range)
    );

    // In range, but inside both the interval and the band → min_interval reports first.
    assert_eq!(
        decide(&no_mute, &json!(0.6), 1_001, Some(&a)),
        Decision::Drop(Reason::MinInterval)
    );

    // Past the interval, still inside the band → deadband.
    assert_eq!(
        decide(&no_mute, &json!(0.6), 20_000, Some(&a)),
        Decision::Drop(Reason::Deadband)
    );
}

#[test]
fn a_clamped_value_is_measured_against_the_deadband_not_the_raw_reading() {
    // Clamp runs BEFORE the deadband, so the stored (clamped) value is what the band compares —
    // otherwise a stuck −9999 sensor would clamp to −40 and re-store it forever.
    let f = Filter {
        deadband: Some(Deadband {
            abs: Some(0.5),
            pct: None,
        }),
        range: Some(Range {
            min: Some(-40.0),
            max: None,
            mode: RangeMode::Clamp,
        }),
        ..Default::default()
    };
    let a = anchor(1, -40.0);
    assert_eq!(
        decide(&f, &json!(-9999.0), 2, Some(&a)),
        Decision::Drop(Reason::Deadband)
    );
}

#[test]
fn an_absent_filter_block_stores_everything() {
    let inert = Filter::default();
    assert!(inert.is_inert());
    assert!(!inert.needs_state());
    assert_eq!(decide(&inert, &json!(1.0), 1, None), Decision::Keep);
    assert_eq!(
        decide(&inert, &json!(1.0), 1, Some(&anchor(1, 1.0))),
        Decision::Keep
    );
}

#[test]
fn serde_defaults_keep_an_old_policy_rows_exact_meaning() {
    // The shape a pre-normalize policy row deserializes as.
    let f: Filter = serde_json::from_value(json!({})).unwrap();
    assert!(f.is_inert());
    // A partial block only turns on what it names.
    let f: Filter = serde_json::from_value(json!({"deadband": {"abs": 0.5}})).unwrap();
    assert!(!f.drop);
    assert_eq!(f.min_interval_ms, 0);
    assert!(f.range.is_none());
    assert!(f.needs_state());
    // Range mode defaults to `drop`.
    let f: Filter = serde_json::from_value(json!({"range": {"max": 10.0}})).unwrap();
    assert_eq!(f.range.unwrap().mode, RangeMode::Drop);
}
