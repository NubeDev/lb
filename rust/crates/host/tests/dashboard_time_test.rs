//! The dashboard `time` field (relative-time-range scope) — headless, real store (`mem://`), real
//! write path, the `dashboard_kind_test.rs` shape. `Dashboard.time` follows `width`'s four layers
//! but is VALIDATED on save like `kind`: it must (a) round-trip through save/get as expressions
//! (never resolved instants), (b) survive a plain layout save (preserve-on-omit), (c) refuse an
//! unresolvable expression LOUDLY leaving the stored value untouched, (d) clear on an explicit
//! all-empty pair, and (e) stay absent on a pre-time record (byte-clean additivity).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    dashboard_get, dashboard_save, dashboard_save_meta, DashboardError, DashboardTime, PageMeta,
};
use lb_store::Store;

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

const ALL: &[&str] = &["mcp:dashboard.get:call", "mcp:dashboard.save:call"];

fn time(from: &str, to: &str) -> Option<DashboardTime> {
    Some(DashboardTime {
        from: from.into(),
        to: to.into(),
    })
}

async fn save_time(
    store: &Store,
    p: &Principal,
    ws: &str,
    t: Option<DashboardTime>,
    now: u64,
) -> Result<(), DashboardError> {
    dashboard_save_meta(
        store,
        p,
        ws,
        "ops",
        "Ops",
        PageMeta {
            time: t,
            ..PageMeta::default()
        },
        vec![],
        vec![],
        now,
    )
    .await
    .map(|_| ())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn dashboard_time_round_trips_preserves_validates_and_clears() {
    let ws = "ws-dash-time";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, ALL);

    // (e) A board created with NO time carries none — absent, not an empty object.
    dashboard_save(&store, &ada, ws, "ops", "Ops", vec![], vec![], 10)
        .await
        .unwrap();
    let plain = dashboard_get(&store, &ada, ws, "ops").await.unwrap();
    assert!(plain.time.is_none(), "pre-time record stays absent");
    let wire = serde_json::to_value(&plain).unwrap();
    assert!(
        wire.get("time").is_none(),
        "absent time stays OFF the wire (byte-clean additivity)"
    );

    // (a) Set a RELATIVE window — stored as the expression, never a resolved instant.
    save_time(&store, &ada, ws, time("last-7-days", ""), 20)
        .await
        .unwrap();
    let got = dashboard_get(&store, &ada, ws, "ops").await.unwrap();
    assert_eq!(got.time.as_ref().unwrap().from, "last-7-days");
    assert_eq!(got.time.as_ref().unwrap().to, "");

    // (b) A plain layout save sends no time — the window must survive, or the first drag
    // silently resets every board's default range.
    dashboard_save(&store, &ada, ws, "ops", "Ops", vec![], vec![], 30)
        .await
        .unwrap();
    let got = dashboard_get(&store, &ada, ws, "ops").await.unwrap();
    assert_eq!(
        got.time.as_ref().unwrap().from,
        "last-7-days",
        "preserve-on-omit holds for time"
    );

    // An endpoint pair round-trips too.
    save_time(&store, &ada, ws, time("now-1d/d", "now/d"), 40)
        .await
        .unwrap();
    let got = dashboard_get(&store, &ada, ws, "ops").await.unwrap();
    assert_eq!(got.time.as_ref().unwrap().to, "now/d");

    // (c) An unresolvable expression is refused LOUDLY, naming the token — and the stored value
    // is untouched (the save never reached the write).
    let err = save_time(&store, &ada, ws, time("nope", ""), 50)
        .await
        .unwrap_err();
    assert!(
        matches!(err, DashboardError::BadInput(ref m) if m.contains("nope")),
        "bad expression must be a loud BadInput naming the token, got {err:?}"
    );
    // A range token with a `to` is the shape refusal.
    let err = save_time(&store, &ada, ws, time("this-month", "now"), 51)
        .await
        .unwrap_err();
    assert!(matches!(err, DashboardError::BadInput(ref m) if m.contains("this-month")));
    let got = dashboard_get(&store, &ada, ws, "ops").await.unwrap();
    assert_eq!(
        got.time.as_ref().unwrap().from,
        "now-1d/d",
        "a refused save leaves the stored window untouched"
    );

    // (d) An explicit all-empty pair CLEARS (an author must be able to remove the default).
    save_time(&store, &ada, ws, time("", ""), 60).await.unwrap();
    let got = dashboard_get(&store, &ada, ws, "ops").await.unwrap();
    assert!(
        got.time.is_none(),
        "an all-empty pair is the explicit clear"
    );
}
