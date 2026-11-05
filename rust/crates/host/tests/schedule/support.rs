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
