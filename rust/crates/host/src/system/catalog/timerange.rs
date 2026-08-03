//! The `time.*` family — the relative time-range resolver (dashboard relative-time-range scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const TIMERANGE: &[HostTool] = &[HostTool {
    tool: "time.range.resolve",
    group: "time",
    description:
        "resolve a relative time-range expression (today, this-month, last-3-months, \
                  now-4h, …) against a clock + timezone into a concrete {fromMs, toMs} window plus \
                  its ISO-day projection — read-only compute, the one canonical calendar arithmetic",
}];
