//! Shared fixtures for the schedule suite.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};

/// The store-surface cap `schedule.get` / `list` / `evaluate` check.
pub const READ: &str = "store:schedule:read";

/// A principal in `ws` holding exactly `caps`.
pub fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:schedules".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|c| c.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// The design doc's §3 "Offices" record (Australia/Sydney), without a `rev`.
pub fn offices() -> serde_json::Value {
    serde_json::from_str(include_str!("offices.json")).expect("fixture parses")
}

/// The store-surface cap `schedule.save` / `delete` check.
pub const WRITE: &str = "store:schedule:write";

/// `offices()` with `rev` set, ready for `schedule.save`.
pub fn offices_at(rev: u64) -> serde_json::Value {
    let mut v = offices();
    v["rev"] = rev.into();
    v
}

/// `schedule.save` as the verb runs it: parse the input, then save as a principal holding both caps.
pub async fn save(
    store: &lb_store::Store,
    ws: &str,
    body: serde_json::Value,
) -> Result<lb_host::SavedSchedule, lb_host::FlowsError> {
    let p = principal(ws, &[READ, WRITE]);
    lb_host::schedule_save(store, &p, ws, &lb_host::ScheduleSave::from_input(&body)?).await
}

/// The message of a `BadInput` refusal, or a panic naming what came back instead.
pub fn bad_input<T: std::fmt::Debug>(r: Result<T, lb_host::FlowsError>) -> String {
    match r {
        Err(lb_host::FlowsError::BadInput(m)) => m,
        other => panic!("expected a BadInput refusal, got {other:?}"),
    }
}
