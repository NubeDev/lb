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
        description: "export a report-kind dashboard to branded PDF. Over the JSON bridge it trades \
                      MEDIA IDS, not bytes: { id, snapshotMediaId?, options? } -> { pdfMediaId, \
                      bytes, mime }, snapshots up via media.upload_*, PDF down via media.read. The \
                      binary route POST /reports/{id}/export.pdf remains the path for callers that \
                      can set an Authorization header, and takes the same optional options. \
                      options: { paper (a4|a3|a5|letter|legal|tabloid), orientation \
                      (portrait|landscape), marginXMm, marginTopMm, marginBottomMm, scale, \
                      pageNumbers, index } — every field optional; omitted composes the shipped A4 \
                      portrait document byte-for-byte. An unknown paper/orientation is a 400 naming \
                      the field, never a silent A4. Own cap: mcp:report.export:call",
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
