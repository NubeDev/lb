//! The `telemetry.*` family — the redacted dispatch/telemetry log (observability scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const TELEMETRY: &[HostTool] = &[
    // telemetry.* — the redacted dispatch/telemetry log (observability scope).
    HostTool {
        tool: "telemetry.query",
        group: "telemetry",
        description: "query the redacted telemetry log",
    },
    HostTool {
        tool: "telemetry.tail",
        group: "telemetry",
        description: "tail recent telemetry events",
    },
    HostTool {
        tool: "telemetry.trace",
        group: "telemetry",
        description: "the events of one trace id",
    },
    HostTool {
        tool: "telemetry.purge",
        group: "telemetry",
        description: "purge telemetry rows (admin)",
    },
];
