//! `schedule.save` and `rev` (design §5): the check and the write are one transaction, `0` or no
//! `rev` creates, a mismatch is refused, an existing id is not overwritten without its `rev`.

use lb_host::{
    schedule_get, schedule_list, schedule_save, FlowsError, SavedSchedule, ScheduleSave,
    UnreadableSchedule,
};
use lb_store::Store;
use serde_json::json;

use super::support::{bad_input, offices, offices_at, principal, save, READ};

#[tokio::test]
async fn rev_0_creates_and_the_current_rev_updates() {
    let store = Store::memory().await.unwrap();
    let ws = "ws-save-rev";
    let created = save(&store, ws, offices_at(0)).await.unwrap();
    assert_eq!(
        created,
        SavedSchedule {
            id: "office-hours".into(),
            rev: 1
        }
    );
    let mut renamed = offices_at(1);
    renamed["name"] = json!("Renamed");
    assert_eq!(save(&store, ws, renamed).await.unwrap().rev, 2);
    let got = schedule_get(&store, &principal(ws, &[READ]), ws, "office-hours")
        .await
        .unwrap();
    assert_eq!(got.name, "Renamed");
    assert_eq!(got.rev, Some(2));
}

#[tokio::test]
async fn a_save_without_rev_creates_once() {
    let store = Store::memory().await.unwrap();
    let ws = "ws-save-norev";
    assert_eq!(save(&store, ws, offices()).await.unwrap().rev, 1);
    assert_eq!(
        bad_input(save(&store, ws, offices()).await),
        "schedule_exists: schedule `office-hours` already exists; send its rev from schedule.get"
    );
}

/// An old-shape row (design §3) is found through `unreadable` and saved over with its `rev`.
#[tokio::test]
async fn an_old_shape_row_is_saved_over_with_its_rev() {
    let store = Store::memory().await.unwrap();
    let ws = "ws-save-heal";
    let old = json!({ "id": "office-hours", "weekly": [], "exceptions": [] });
    lb_store::write(&store, ws, "schedule", "office-hours", &old)
        .await
        .unwrap();
    let p = principal(ws, &[READ]);
    let listed = schedule_list(&store, &p, ws).await.unwrap();
    assert_eq!(
        listed.unreadable,
        vec![UnreadableSchedule {
            id: "office-hours".into(),
            rev: 1
        }]
    );
    assert_eq!(
        bad_input(save(&store, ws, offices()).await),
        "schedule_exists: schedule `office-hours` already exists; send its rev from schedule.get"
    );
    assert_eq!(save(&store, ws, offices_at(1)).await.unwrap().rev, 2);
    assert_eq!(
        schedule_get(&store, &p, ws, "office-hours")
            .await
            .unwrap()
            .name,
        "Offices"
    );
    assert!(schedule_list(&store, &p, ws)
        .await
        .unwrap()
        .unreadable
        .is_empty());
}

#[tokio::test]
async fn a_stale_rev_is_refused_and_changes_nothing() {
    let store = Store::memory().await.unwrap();
    let ws = "ws-save-stale";
    save(&store, ws, offices_at(0)).await.unwrap();
    let stale = "schedule_stale: schedule `office-hours` changed elsewhere, reload";
    let mut second = offices_at(0);
    second["name"] = json!("Second");
    assert_eq!(
        bad_input(save(&store, ws, second.clone()).await),
        "schedule_exists: schedule `office-hours` already exists; send its rev from schedule.get",
        "rev 0 is a create, like no rev"
    );
    second["rev"] = json!(7);
    assert_eq!(bad_input(save(&store, ws, second).await), stale);
    assert_eq!(
        bad_input(save(&store, ws, json!({ "id": "missing", "rev": 3 })).await),
        "schedule_stale: schedule `missing` changed elsewhere, reload"
    );
    let got = schedule_get(&store, &principal(ws, &[READ]), ws, "office-hours")
        .await
        .unwrap();
    assert_eq!(got.name, "Offices");
    assert_eq!(got.rev, Some(1));
}

#[tokio::test]
async fn saving_needs_the_write_cap() {
    let store = Store::memory().await.unwrap();
    let ws = "ws-save-deny";
    let p = principal(ws, &[READ]);
    let input = ScheduleSave::from_input(&offices_at(0)).unwrap();
    assert!(matches!(
        schedule_save(&store, &p, ws, &input).await,
        Err(FlowsError::Denied)
    ));
}

#[tokio::test]
async fn each_workspace_keeps_its_own_revisions() {
    let store = Store::memory().await.unwrap();
    assert_eq!(
        save(&store, "ws-save-a", offices_at(0)).await.unwrap().rev,
        1
    );
    assert_eq!(
        save(&store, "ws-save-a", offices_at(1)).await.unwrap().rev,
        2
    );
    assert_eq!(
        save(&store, "ws-save-b", offices_at(0)).await.unwrap().rev,
        1
    );
    let b = schedule_get(
        &store,
        &principal("ws-save-b", &[READ]),
        "ws-save-b",
        "office-hours",
    )
    .await
    .unwrap();
    assert_eq!(b.rev, Some(1));
}
