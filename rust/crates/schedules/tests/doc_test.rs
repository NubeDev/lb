//! The schedule document keeps rubix-ce's names and fills what rubix-ce leaves out (design §3).

use lb_schedules::ScheduleDoc;
use serde_json::json;

fn offices() -> ScheduleDoc {
    serde_json::from_str(include_str!("fixtures/offices_doc.json")).expect("fixture parses")
}

#[test]
fn the_offices_document_round_trips_with_its_additions() {
    let doc = offices();
    let back: ScheduleDoc = serde_json::from_value(serde_json::to_value(&doc).unwrap()).unwrap();
    assert_eq!(doc, back);
    let x = &doc.schedules.exception;
    assert!(x["x2"].yearly);
    assert_eq!(x["x1"].kind.as_deref(), Some("changed"));
    assert_eq!(x["h1"].label.as_deref(), Some("Christmas Day"));
    assert_eq!(doc.schedules.weekly["w1"].value, 2.0);
}

#[test]
fn defaults_fill_what_rubix_ce_leaves_out() {
    let doc: ScheduleDoc = serde_json::from_value(json!({
        "schedules": { "weekly": { "w1": {
            "name": "Default", "days": ["monday"], "start": "09:00", "end": "16:59"
        } } }
    }))
    .unwrap();
    assert_eq!(doc.config.names, vec!["Default".to_string()]);
    assert_eq!(doc.config.default_name, "Default");
    let w = &doc.schedules.weekly["w1"];
    assert!(w.enable, "a missing enable means true");
    assert_eq!(w.value, 0.0, "a missing value means 0");
}

#[test]
fn screen_settings_are_not_kept() {
    let doc: ScheduleDoc = serde_json::from_value(json!({
        "config": {
            "names": ["Occupied"], "default_name": "Occupied",
            "input_type": "number", "step": 1, "input_name": "x", "disable_weekly": false
        }
    }))
    .unwrap();
    let out = serde_json::to_value(&doc).unwrap();
    assert_eq!(
        out["config"],
        json!({ "names": ["Occupied"], "default_name": "Occupied" })
    );
}
