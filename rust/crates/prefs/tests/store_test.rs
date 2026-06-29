//! The store layer over a REAL `mem://` store (no mocks): set/get round-trip, MERGE patch
//! semantics, the canonical guarantee (no formatted string persisted), resolve-from-store, and the
//! offline idempotent replay (composite-id upsert, LWW).

use lb_prefs::{
    get_user_prefs, get_workspace_prefs, resolve_chain, set_user_prefs, set_workspace_prefs,
    DateStyle, Dimension, NumberFormat, Prefs, TimeStyle, Unit, UnitSystem, USER_PREFS_TABLE,
};
use lb_store::Store;

fn seed_user() -> Prefs {
    let mut p = Prefs {
        language: Some("es".into()),
        timezone: Some("Europe/Madrid".into()),
        date_style: Some(DateStyle::Eu),
        time_style: Some(TimeStyle::H24),
        number_format: Some(NumberFormat::CommaDot),
        unit_system: Some(UnitSystem::Metric),
        ..Prefs::default()
    };
    p.unit_overrides.insert(Dimension::Speed, Unit::Knot);
    p
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn set_then_get_round_trips_canonical() {
    let store = Store::memory().await.unwrap();
    let p = seed_user();
    set_user_prefs(&store, "acme", "user:ada", &p)
        .await
        .unwrap();

    let got = get_user_prefs(&store, "acme", "user:ada")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got, p, "the stored prefs round-trip unchanged");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn patch_merge_leaves_other_axes_untouched() {
    let store = Store::memory().await.unwrap();
    set_user_prefs(&store, "acme", "user:ada", &seed_user())
        .await
        .unwrap();

    // A patch that only changes date_style must NOT clear language/timezone/etc.
    let patch = Prefs {
        date_style: Some(DateStyle::Usa),
        ..Prefs::default()
    };
    set_user_prefs(&store, "acme", "user:ada", &patch)
        .await
        .unwrap();

    let got = get_user_prefs(&store, "acme", "user:ada")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.date_style, Some(DateStyle::Usa));
    assert_eq!(got.language, Some("es".to_string())); // untouched
    assert_eq!(got.unit_overrides.get(&Dimension::Speed), Some(&Unit::Knot));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn no_formatted_string_is_persisted() {
    // Canonical guarantee: the stored row carries only locale-neutral enums/ids, never a rendered
    // string like "43,2 km/h" or "27/06/2026". Read the raw row and assert its values are canonical.
    let store = Store::memory().await.unwrap();
    set_user_prefs(&store, "acme", "user:ada", &seed_user())
        .await
        .unwrap();

    let mut resp = store
        .query_ws(
            "acme",
            &format!(
                "SELECT language, timezone, date_style, unit_overrides FROM \
                 type::thing('{USER_PREFS_TABLE}', [$ws, $user])"
            ),
            vec![
                ("ws".into(), "acme".into()),
                ("user".into(), "user:ada".into()),
            ],
        )
        .await
        .unwrap();
    let rows: Vec<serde_json::Value> = resp.take(0).unwrap();
    let row = rows.into_iter().next().unwrap();
    // timezone is an IANA id (canonical), date_style is the enum token — never a formatted date.
    assert_eq!(row["timezone"], serde_json::json!("Europe/Madrid"));
    assert_eq!(row["date_style"], serde_json::json!("eu"));
    let blob = row.to_string();
    assert!(!blob.contains("43,2"), "no formatted number persisted");
    // No formatted date (e.g. "27/06/2026"); the IANA tz id legitimately contains a slash so we
    // assert on the date pattern, not bare slashes.
    assert!(
        !blob.contains("/2026"),
        "no formatted date persisted: {blob}"
    );
    assert!(
        !blob.contains("/06/"),
        "no formatted date persisted: {blob}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn resolve_from_store_folds_user_over_workspace_default() {
    let store = Store::memory().await.unwrap();
    // workspace default: metric, Madrid. user: knots override only.
    set_workspace_prefs(
        &store,
        "acme",
        &Prefs {
            unit_system: Some(UnitSystem::Metric),
            timezone: Some("Europe/Madrid".into()),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();
    let mut user = Prefs::default();
    user.unit_overrides.insert(Dimension::Speed, Unit::Knot);
    set_user_prefs(&store, "acme", "user:ada", &user)
        .await
        .unwrap();

    let r = resolve_chain(&store, "acme", "user:ada", None)
        .await
        .unwrap();
    assert_eq!(r.timezone, "Europe/Madrid"); // from ws default
    assert_eq!(r.unit_system, UnitSystem::Metric); // from ws default
    assert_eq!(r.unit_overrides.get(&Dimension::Speed), Some(&Unit::Knot)); // from user

    // A per-call override wins without writing the record.
    let preview = Prefs {
        timezone: Some("Asia/Tokyo".into()),
        ..Prefs::default()
    };
    let r2 = resolve_chain(&store, "acme", "user:ada", Some(preview))
        .await
        .unwrap();
    assert_eq!(r2.timezone, "Asia/Tokyo");
    // the stored record is unchanged by the preview.
    let stored = get_user_prefs(&store, "acme", "user:ada")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.timezone, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn offline_edit_replays_idempotently() {
    // Composite-id upsert: applying the SAME edit twice (an offline edit that replays on reconnect)
    // yields ONE record with the last value — never a duplicate.
    let store = Store::memory().await.unwrap();
    let edit = Prefs {
        language: Some("es".into()),
        ..Prefs::default()
    };
    set_user_prefs(&store, "acme", "user:ada", &edit)
        .await
        .unwrap();
    set_user_prefs(&store, "acme", "user:ada", &edit)
        .await
        .unwrap(); // replay

    let mut resp = store
        .query_ws(
            "acme",
            &format!("SELECT count() FROM {USER_PREFS_TABLE} GROUP ALL"),
            vec![],
        )
        .await
        .unwrap();
    let n: Option<i64> = resp.take("count").unwrap();
    assert_eq!(
        n.unwrap_or(0),
        1,
        "replay upserts in place, no duplicate record"
    );

    // LWW: a later edit of the same axis wins.
    let edit2 = Prefs {
        language: Some("en".into()),
        ..Prefs::default()
    };
    set_user_prefs(&store, "acme", "user:ada", &edit2)
        .await
        .unwrap();
    let got = get_user_prefs(&store, "acme", "user:ada")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.language, Some("en".to_string()));
}
