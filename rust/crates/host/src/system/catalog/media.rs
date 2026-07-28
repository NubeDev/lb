//! The `media.*` family — the chunked-upload + variant + serve surface (media scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const MEDIA: &[HostTool] = &[
    // media.* — the chunked-upload + variant + serve surface (media scope).
    HostTool {
        tool: "media.upload_begin",
        group: "media",
        description: "begin a resumable chunked upload (declares size/mime/checksum)",
    },
    HostTool {
        tool: "media.upload_commit",
        group: "media",
        description: "commit an upload (verify checksum, derive variants, flip to ready)",
    },
    HostTool {
        tool: "media.get",
        group: "media",
        description: "read media metadata by id",
    },
    HostTool {
        tool: "media.list",
        group: "media",
        description: "list media in the workspace",
    },
    HostTool {
        tool: "media.delete",
        group: "media",
        description: "archive media by id",
    },
];
