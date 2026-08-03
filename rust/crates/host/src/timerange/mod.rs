//! The **relative time-range grammar** (dashboard relative-time-range scope) — one canonical
//! parser + resolver for `today` / `yesterday` / `this-month` / `last-3-months` / `now-4h` …, so a
//! dashboard default window, a schedule payload and a flow all name a window instead of freezing
//! it. Resolution is pure arithmetic over `(expression, now, tz)` — the clock and the timezone are
//! parameters, never read here (symmetric nodes).
//!
//! One responsibility per file:
//!   - `grammar` — `parse`: what a string IS (endpoint vs range token), and every refusal.
//!   - `civil` — the Hinnant calendar maths (exact proleptic-Gregorian day↔date, clamped months).
//!   - `resolve` — `(from, to?, now_ms, tz)` → `{from_ms, to_ms}` + the ISO-day projection.
//!   - `tool` — the `time.range.resolve` MCP verb (gated `mcp:time.range.resolve:call`).
//!
//! **There is no legacy-preset compat layer, by decision.** The seven pre-grammar report preset ids
//! (`yesterday`, `last-24-hours`, `last-7-days`, …) once had a mapping module here; nothing is in
//! production carrying them, so keeping a second vocabulary alive would have bought only drift.
//! This grammar is the ONLY window vocabulary — `last-7-days` is now a grammar token that means what
//! the grammar says, not a preset id with its own arithmetic.

mod civil;
mod grammar;
mod resolve;
mod tool;

pub use grammar::{
    parse, CalUnit, Endpoint, EndpointBase, RangeExpr, TimeRangeError, Unit, Window,
};
pub use resolve::{parse_tz, resolve, resolve_range, validate, ResolvedRange};
pub use tool::{call_timerange_tool, resolve_descriptor};
