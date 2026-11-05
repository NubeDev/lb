//! `schedule.get` / `schedule.list` return the store `rev` (design §5). A stored row in the old shape
//! is refused by `get` and `evaluate` and listed under `unreadable` (design §3).

use lb_host::{schedule_evaluate, schedule_get, schedule_list, FlowsError, UnreadableSchedule};
use lb_store::{write, Store};
use serde_json::json;

use super::support::{offices, principal, READ};

/// Seed through the real store write path, the way `schedule.save` lands a record.
async fn seed(store: &Store, ws: &str, body: serde_json::Value) {
    let id = body["id"].as_str().unwrap().to_string();
    write(store, ws, "schedule", &id, &body).await.unwrap();
}

/// A row as the old lb wrote it: top-level `weekly` / `exceptions`, no `schedule` document.
fn old_shape(id: &str) -> serde_json::Value {
    json!({
        "id": id, "name": "Old", "use_utc": true,
        "weekly": [{ "day": 1, "time_ranges": [{ "start": "00:00", "stop": "23:59" }] }],
        "exceptions": [{ "start": "2026-12-25T00:00:00Z", "stop": "2026-12-26T00:00:00Z",
                         "exception_type": "holiday", "priority": 50 }]
    })
}

fn bad_input<T: std::fmt::Debug>(r: Result<T, FlowsError>) -> String {
    match r {
        Err(FlowsError::BadInput(m)) => m,
        other => panic!("expected a BadInput refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn get_and_list_carry_rev() {
    let store = Store::memory().await.unwrap();
    let ws = "ws-sched-read";
    seed(&store, ws, offices()).await;
    seed(&store, ws, offices()).await;
    let p = principal(ws, &[READ]);

    let got = schedule_get(&store, &p, ws, "office-hours").await.unwrap();
    assert_eq!(got.rev, Some(2));
    assert_eq!(got.name, "Offices");
    assert_eq!(got.schedule.schedules.weekly.len(), 2);

    let listed = schedule_list(&store, &p, ws).await.unwrap();
    assert_eq!(listed.schedules.len(), 1);
    assert_eq!(listed.schedules[0].rev, Some(2));
    assert!(listed.unreadable.is_empty());
}

#[tokio::test]
async fn an_old_shape_row_is_refused_and_listed_as_unreadable() {
    let store = Store::memory().await.unwrap();
    let ws = "ws-sched-old";
    seed(&store, ws, old_shape("old")).await;
    seed(&store, ws, old_shape("old")).await;
    seed(&store, ws, offices()).await;
    let p = principal(ws, &[READ]);
    let refusal = "schedule_old_shape: schedule `old` is in the old shape (no `schedule` document); save over it with rev 2 or delete it";

    assert_eq!(
        bad_input(schedule_get(&store, &p, ws, "old").await),
        refusal
    );
    assert_eq!(
        bad_input(schedule_evaluate(&store, &p, ws, "old").await),
        refusal
    );

    let listed = schedule_list(&store, &p, ws).await.unwrap();
    assert_eq!(
        listed.schedules.len(),
        1,
        "the readable schedule is still listed"
    );
    assert_eq!(listed.schedules[0].id, "office-hours");
    assert_eq!(
        listed.unreadable,
        vec![UnreadableSchedule {
            id: "old".into(),
            rev: 2
        }]
    );
}

#[tokio::test]
async fn a_row_that_does_not_decode_is_listed_as_unreadable() {
    let store = Store::memory().await.unwrap();
    let ws = "ws-sched-garbage";
    seed(
        &store,
        ws,
        json!({ "id": "bad", "schedule": { "schedules": { "weekly": "not a map" } } }),
    )
    .await;
    let listed = schedule_list(&store, &principal(ws, &[READ]), ws)
        .await
        .unwrap();
    assert!(listed.schedules.is_empty());
    assert_eq!(
        listed.unreadable,
        vec![UnreadableSchedule {
            id: "bad".into(),
            rev: 1
        }]
    );
}

#[tokio::test]
async fn reading_needs_the_read_cap() {
    let store = Store::memory().await.unwrap();
    let ws = "ws-sched-deny";
    seed(&store, ws, offices()).await;
    let p = principal(ws, &[]);
    assert!(matches!(
        schedule_get(&store, &p, ws, "office-hours").await,
        Err(FlowsError::Denied)
    ));
    assert!(matches!(
        schedule_list(&store, &p, ws).await,
        Err(FlowsError::Denied)
    ));
}

#[tokio::test]
async fn another_workspace_cannot_see_the_record() {
    let store = Store::memory().await.unwrap();
    seed(&store, "ws-sched-a", offices()).await;
    let b = principal("ws-sched-b", &[READ]);
    assert_eq!(
        bad_input(schedule_get(&store, &b, "ws-sched-b", "office-hours").await),
        "schedule `office-hours` not found"
    );
    let listed = schedule_list(&store, &b, "ws-sched-b").await.unwrap();
    assert!(listed.schedules.is_empty());
    assert!(listed.unreadable.is_empty());
}

/// A row written before the store kept revisions (no `rev` at all) reads as rev 1: `get` names it
/// and `list` lists it, rather than the whole call failing to decode.
#[tokio::test]
async fn an_old_row_without_a_rev_is_named_with_rev_1() {
    let store = Store::memory().await.unwrap();
    let ws = "ws-sched-norev";
    store
        .query_ws(
            ws,
            "UPSERT type::record('schedule', 'legacy') CONTENT { data: { id: 'legacy', weekly: [] } } RETURN NONE",
            vec![],
        )
        .await
        .unwrap();
    let p = principal(ws, &[READ]);
    let refusal = bad_input(schedule_get(&store, &p, ws, "legacy").await);
    assert!(
        refusal.ends_with("save over it with rev 1 or delete it"),
        "{refusal}"
    );
    let listed = schedule_list(&store, &p, ws).await.unwrap();
    assert_eq!(
        listed.unreadable,
        vec![UnreadableSchedule {
            id: "legacy".into(),
            rev: 1
        }]
    );
}
