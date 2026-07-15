//! Test support — a real rhai engine with the real `lb_frame::register` surface (no mocks; the
//! fixture rows are seeded through the same `frame(records)` path an author uses).
#![allow(dead_code)] // each test binary uses a different subset of these helpers

use lb_frame::FrameLimits;
use rhai::{Dynamic, Engine};
use serde_json::Value;

/// A fresh engine with the Frame surface registered under `limits`.
pub fn engine_with(limits: FrameLimits) -> Engine {
    let mut engine = Engine::new();
    lb_frame::register(&mut engine, &limits);
    engine
}

/// A fresh engine with the default limits.
pub fn engine() -> Engine {
    engine_with(FrameLimits::default())
}

/// The shared fixture rows, as a rhai snippet: 6 rows, two series, one null value.
/// Prepend to a script body via [`eval`]'s `FIXTURE` helper.
pub const FIXTURE: &str = r#"
let recs = [
    #{ ts: 100, series: "a", value: 3.0 },
    #{ ts: 200, series: "a", value: 7.0 },
    #{ ts: 300, series: "a", value: () },
    #{ ts: 100, series: "b", value: 5.0 },
    #{ ts: 200, series: "b", value: 1.0 },
    #{ ts: 300, series: "b", value: 9.0 },
];
let f = frame(recs);
"#;

/// Evaluate a script and return the result as JSON (rhai -> serde through the engine's own
/// serde bridge, so what a rule body would see is what the test asserts on).
pub fn eval_json(engine: &Engine, script: &str) -> Result<Value, String> {
    let d = engine.eval::<Dynamic>(script).map_err(|e| e.to_string())?;
    rhai::serde::from_dynamic::<Value>(&d).map_err(|e| e.to_string())
}

/// Evaluate `FIXTURE` + the body (which can use `f`) and return JSON.
pub fn eval_fixture(engine: &Engine, body: &str) -> Result<Value, String> {
    eval_json(engine, &format!("{FIXTURE}\n{body}"))
}

/// Assert the script errors and return the error text.
pub fn eval_err(engine: &Engine, script: &str) -> String {
    match engine.eval::<Dynamic>(script) {
        Ok(v) => panic!("expected an error, got: {v:?}"),
        Err(e) => e.to_string(),
    }
}
