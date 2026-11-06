//! What a save takes: the document, validated (design §3), and never the old top-level fields
//! (design §5).

use lb_host::schedule_get;
use lb_store::Store;
use serde_json::json;

use super::support::{bad_input, offices, offices_at, principal, save, READ};

const LEGACY: &str = "schedule_legacy_fields: schedule `office-hours` no longer takes `weekly` or `exceptions`; edit `schedule` instead";

#[tokio::test]
async fn the_saved_document_reads_back() {
    let store = Store::memory().await.unwrap();
    let ws = "ws-shape-roundtrip";
    save(&store, ws, offices_at(0)).await.unwrap();
    let got = schedule_get(&store, &principal(ws, &[READ]), ws, "office-hours")
        .await
        .unwrap();
    let sent: lb_schedules::ScheduleDoc =
        serde_json::from_value(offices()["schedule"].clone()).unwrap();
    assert_eq!(got.schedule, sent);
    assert_eq!(got.max_payload, 2.0);
    let out = serde_json::to_value(&got).unwrap();
    assert!(out.get("weekly").is_none());
    assert!(out.get("exceptions").is_none());
}

#[tokio::test]
async fn the_old_top_level_fields_are_refused() {
    let store = Store::memory().await.unwrap();
    let ws = "ws-shape-legacy";
    let mut v = offices_at(0);
    v["weekly"] = json!([{ "day": 1, "time_ranges": [{ "start": "09:00", "stop": "17:00" }] }]);
    assert_eq!(bad_input(save(&store, ws, v).await), LEGACY);
    let mut v = offices_at(0);
    v["exceptions"] = json!(null);
    assert_eq!(bad_input(save(&store, ws, v).await), LEGACY);
}

#[tokio::test]
async fn an_out_of_range_value_is_refused_naming_the_entry() {
    let store = Store::memory().await.unwrap();
    let mut v = offices_at(0);
    v["schedule"]["schedules"]["weekly"]["w1"]["value"] = json!(3);
    assert_eq!(
        bad_input(save(&store, "ws-shape-range", v).await),
        "schedule `office-hours`: weekly `w1`: value 3 is outside 0..=2"
    );
}

#[tokio::test]
async fn yearly_only_on_closures_and_holidays() {
    let store = Store::memory().await.unwrap();
    let mut v = offices_at(0);
    v["schedule"]["schedules"]["exception"]["x1"]["yearly"] = json!(true);
    assert_eq!(
        bad_input(save(&store, "ws-shape-yearly", v).await),
        "schedule `office-hours`: exception `x1`: `yearly` is allowed only on `closed` and `holiday` entries"
    );
}

#[tokio::test]
async fn min_payload_above_max_payload_is_refused() {
    let store = Store::memory().await.unwrap();
    let mut v = offices_at(0);
    v["min_payload"] = json!(3);
    assert_eq!(
        bad_input(save(&store, "ws-shape-minmax", v).await),
        "schedule `office-hours`: min_payload 3 is above max_payload 2"
    );
}

#[tokio::test]
async fn a_bad_zone_is_refused() {
    let store = Store::memory().await.unwrap();
    let mut v = offices_at(0);
    v["timezone"] = json!("Mars/Olympus");
    assert_eq!(
        bad_input(save(&store, "ws-shape-zone", v).await),
        "schedule `office-hours`: unknown timezone: Mars/Olympus"
    );
}

#[tokio::test]
async fn an_empty_id_is_refused() {
    let store = Store::memory().await.unwrap();
    assert_eq!(
        bad_input(save(&store, "ws-shape-id", json!({ "id": " ", "rev": 0 })).await),
        "schedule needs a non-empty `id`"
    );
}
