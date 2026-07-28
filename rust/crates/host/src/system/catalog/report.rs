//! The `report.*` builder asset + the `brand.*` reusable brand profile (reports scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const REPORT: &[HostTool] = &[
    // report.* — the report-builder asset (reports scope).
    HostTool {
        tool: "report.get",
        group: "report",
        description: "read one report by id (blocks hydrated)",
    },
    HostTool {
        tool: "report.list",
        group: "report",
        description: "list the reports visible to the caller",
    },
    HostTool {
        tool: "report.save",
        group: "report",
        description: "create or update a report the caller owns",
    },
    HostTool {
        tool: "report.delete",
        group: "report",
        description: "delete a report the caller owns",
    },
    HostTool {
        tool: "report.share",
        group: "report",
        description: "share a report with a team / set its visibility",
    },
    HostTool {
        tool: "report.export",
        group: "report",
        description: "export a report to branded PDF (gateway binary route; own cap)",
    },
    // brand.* — the reusable brand-profile asset (reports scope).
    HostTool {
        tool: "brand.get",
        group: "brand",
        description: "read one brand profile by id",
    },
    HostTool {
        tool: "brand.list",
        group: "brand",
        description: "list the brand profiles in the workspace",
    },
    HostTool {
        tool: "brand.save",
        group: "brand",
        description: "create or update a brand profile the caller owns",
    },
    HostTool {
        tool: "brand.delete",
        group: "brand",
        description: "delete a brand profile the caller owns",
    },
];
