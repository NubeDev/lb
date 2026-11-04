//! The save-time rules for a schedule document (design §3, "Validation on save").

use lb_schedules::{validate, PayloadLimits, ScheduleDoc};
use serde_json::{json, Value};

const LIMITS: PayloadLimits = PayloadLimits {
    enable_payload: true,
    min_payload: 0.0,
    max_payload: 2.0,
};

fn offices() -> Value {
    serde_json::from_str(include_str!("fixtures/offices_doc.json")).unwrap()
}

fn doc(v: Value) -> ScheduleDoc {
    serde_json::from_value(v).unwrap()
}

fn problem(v: Value, limits: &PayloadLimits) -> String {
    validate(&doc(v), limits)
        .expect_err("should be refused")
        .to_string()
}

#[test]
fn the_offices_document_is_valid() {
    validate(&doc(offices()), &LIMITS).unwrap();
}

#[test]
fn min_payload_may_not_exceed_max_payload() {
    let upside_down = PayloadLimits {
        enable_payload: false,
        min_payload: 3.0,
        max_payload: 2.0,
    };
    assert_eq!(
        problem(offices(), &upside_down),
        "min_payload 3 is above max_payload 2"
    );
}

#[test]
fn weekly_and_event_values_stay_inside_the_payload_range() {
    let mut d = offices();
    d["schedules"]["weekly"]["w1"]["value"] = json!(3);
    assert_eq!(problem(d, &LIMITS), "weekly `w1`: value 3 is outside 0..=2");
    let mut d = offices();
    d["schedules"]["events"]["e1"]["value"] = json!(-1);
    assert_eq!(problem(d, &LIMITS), "event `e1`: value -1 is outside 0..=2");
}

#[test]
fn values_are_free_when_payloads_are_off_and_on_exceptions() {
    let mut d = offices();
    d["schedules"]["weekly"]["w1"]["value"] = json!(7);
    let off = PayloadLimits {
        enable_payload: false,
        ..LIMITS
    };
    validate(&doc(d), &off).unwrap();
    let mut d = offices();
    d["schedules"]["exception"]["x1"]["value"] = json!(9);
    validate(&doc(d), &LIMITS).unwrap();
}

#[test]
fn names_come_from_config_names() {
    let mut d = offices();
    d["schedules"]["weekly"]["w2"]["name"] = json!("Night");
    assert_eq!(
        problem(d, &LIMITS),
        "weekly `w2`: name `Night` is not in config.names [\"Occupied\", \"Standby\"]"
    );
}

#[test]
fn a_weekly_entry_stays_inside_one_day() {
    let mut d = offices();
    d["schedules"]["weekly"]["w1"]["start"] = json!("18:00");
    assert_eq!(
        problem(d, &LIMITS),
        "weekly `w1`: start `18:00` is after end `06:59`; a weekly entry cannot cross midnight"
    );
    let mut d = offices();
    d["schedules"]["weekly"]["w1"]["end"] = json!("24:00");
    assert_eq!(
        problem(d, &LIMITS),
        "weekly `w1`: invalid time format '24:00': expected HH:MM"
    );
}

#[test]
fn days_are_lowercase_day_names() {
    let mut d = offices();
    d["schedules"]["weekly"]["w1"]["days"] = json!(["Monday"]);
    assert_eq!(problem(d, &LIMITS), "weekly `w1`: unknown day `Monday`");
}

#[test]
fn dated_windows_are_real_and_forward() {
    let mut d = offices();
    d["schedules"]["events"]["e1"]["dates"] =
        json!([{ "start": "2026-11-21T23:00", "end": "2026-11-21T21:00" }]);
    assert_eq!(
        problem(d, &LIMITS),
        "event `e1`: start `2026-11-21T23:00` is not before end `2026-11-21T21:00`"
    );
    let mut d = offices();
    d["schedules"]["exception"]["x1"]["dates"][0]["start"] = json!("2026-02-30T06:30");
    let m = problem(d, &LIMITS);
    assert!(
        m.starts_with("exception `x1`: invalid datetime string '2026-02-30T06:30'"),
        "{m}"
    );
}

#[test]
fn yearly_only_on_closures_and_holidays() {
    let mut d = offices();
    d["schedules"]["exception"]["x1"]["yearly"] = json!(true);
    assert_eq!(
        problem(d, &LIMITS),
        "exception `x1`: `yearly` is allowed only on `closed` and `holiday` entries"
    );
    let mut d = offices();
    d["schedules"]["exception"]["h1"]["yearly"] = json!(true);
    validate(&doc(d), &LIMITS).unwrap();
}

#[test]
fn kinds_are_known() {
    let mut d = offices();
    d["schedules"]["exception"]["x1"]["kind"] = json!("maybe");
    assert_eq!(problem(d, &LIMITS), "exception `x1`: unknown kind `maybe`");
}

#[test]
fn overlaps_are_allowed() {
    let mut d = offices();
    d["schedules"]["weekly"]["w2"]["start"] = json!("06:00");
    validate(&doc(d), &LIMITS).unwrap();
}

#[test]
fn times_have_exactly_the_documented_shape() {
    for bad in ["7:5", "07:5", "+7:05", "24:00", "7:05"] {
        let mut d = offices();
        d["schedules"]["weekly"]["w1"]["start"] = json!(bad);
        let m = problem(d, &LIMITS);
        assert_eq!(
            m,
            format!("weekly `w1`: invalid time format '{bad}': expected HH:MM")
        );
    }
    for bad in [
        "2026-02-30T10:00",
        "2026-02-01T10:00:00",
        "2026-2-1T10:00",
        "2026-02-01T1:00",
        "-0001-01-01T00:00",
        "+10000-01-01T00:00",
    ] {
        let mut d = offices();
        d["schedules"]["events"]["e1"]["dates"] =
            json!([{ "start": bad, "end": "2027-01-01T00:00" }]);
        let m = problem(d, &LIMITS);
        assert!(
            m.starts_with(&format!("event `e1`: invalid datetime string '{bad}'")),
            "{m}"
        );
    }
}
