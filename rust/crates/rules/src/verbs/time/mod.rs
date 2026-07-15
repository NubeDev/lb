//! The `time` scope handle — the run's injected logical clock (data-stdlib-scope). All reads come
//! from the pinned `now` (never a wall-clock): determinism and re-run idempotency hold exactly as
//! rules-messaging-scope requires. Unix **seconds** (i64) unless suffixed `_ms`; all formatting is
//! UTC unless an explicit fixed offset (`"+HH:MM"`) is given. One verb group per file
//! (FILE-LAYOUT); every `register_fn` here has a matching row in `catalog.rs`.

mod arith;
mod bounds;
mod catalog;
mod format;
mod parse;
mod parts;

use chrono::{DateTime, Utc};
use rhai::{Engine, EvalAltResult};

use crate::grid::rhai_err;

pub(crate) use catalog::CATALOG;

/// The `time` handle. `now_ms` is the run's pinned logical clock in milliseconds (the same value
/// messaging ids stamp). It is the ONLY clock in the cage — chrono here does pure math.
#[derive(Clone, Copy)]
pub struct TimeHandle {
    pub now_ms: u64,
}

impl TimeHandle {
    /// The pinned clock in unix seconds — what every secs-based verb measures against.
    pub(crate) fn now_secs(&self) -> i64 {
        (self.now_ms / 1000) as i64
    }
}

/// Register the `time.*` methods. The handle is pushed as the `time` scope var by the engine.
pub fn register(engine: &mut Engine) {
    engine.register_type_with_name::<TimeHandle>("Time");
    engine.register_fn("now", |t: &mut TimeHandle| t.now_secs());
    engine.register_fn("now_ms", |t: &mut TimeHandle| t.now_ms as i64);
    format::register(engine);
    parse::register(engine);
    parts::register(engine);
    bounds::register(engine);
    arith::register(engine);
}

/// Unix seconds → UTC datetime, or an author-facing range error (chrono caps at ~±262000 years —
/// past that the input is a bug, not a date).
pub(crate) fn utc(ts: i64) -> Result<DateTime<Utc>, Box<EvalAltResult>> {
    DateTime::<Utc>::from_timestamp(ts, 0)
        .ok_or_else(|| rhai_err(format!("timestamp {ts} is out of range")))
}
