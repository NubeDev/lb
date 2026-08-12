//! The `nav.*` family — the authored navigation menu asset + the workspace hidden-set.
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const NAV: &[HostTool] = &[
    // nav.* — the user-/team-authored navigation menu asset (nav scope).
    HostTool {
        tool: "nav.get",
        group: "nav",
        description: "read one navigation menu by id",
    },
    HostTool {
        tool: "nav.list",
        group: "nav",
        description: "list the navigation menus visible to the caller",
    },
    HostTool {
        tool: "nav.save",
        group: "nav",
        description: "create or update a navigation menu the caller owns",
    },
    HostTool {
        tool: "nav.delete",
        group: "nav",
        description: "delete a navigation menu the caller owns",
    },
    HostTool {
        tool: "nav.share",
        group: "nav",
        description: "share a navigation menu with a team / set its visibility",
    },
    HostTool {
        tool: "nav.set_default",
        group: "nav",
        description: "set the workspace-default navigation menu",
    },
    HostTool {
        tool: "nav.resolve",
        group: "nav",
        description: "resolve the caller's effective menu (picked, tag-expanded, cap-stripped)",
    },
    HostTool {
        tool: "nav.pref.get",
        group: "nav",
        description: "read the caller's own active-nav pick",
    },
    HostTool {
        tool: "nav.pref.set",
        group: "nav",
        description: "set the caller's own active-nav pick, pinned favorites, and/or the force-built-in override (partial write; the override never touches the pick)",
    },
    // hide-and-pins scope: the workspace hidden-set — the admin's one subtractive sidebar-curation
    // lever (declutter only; hiding never blocks a route).
    HostTool {
        tool: "nav.hidden.get",
        group: "nav",
        description: "read the workspace sidebar hidden-set",
    },
    HostTool {
        tool: "nav.hidden.set",
        group: "nav",
        description: "replace the workspace sidebar hidden-set (admin)",
    },
    HostTool {
        tool: "nav.unshare",
        group: "nav",
        description: "withdraw a nav's share from a team (rides the nav.share cap; owner-only)",
    },
    HostTool {
        tool: "nav.list_shares",
        group: "nav",
        description: "the teams a nav is shared with (rides the nav.share cap; owner-only)",
    },
];
