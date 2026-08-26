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
        tool: "nav.get_default",
        group: "nav",
        description: "read the workspace-default navigation menu pointer",
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
    // The ARRANGING counterpart on the same record — a partial order over the same opaque refs.
    HostTool {
        tool: "nav.order.set",
        group: "nav",
        description: "replace the workspace sidebar ordering (admin; partial order over item refs)",
    },
    // host-authored-ext-nav-boards scope: the host's own board rows inside an extension's section
    // — placing a board under an extension without an extension release. Read is member-level
    // (rides nav.resolve); the write rides nav.save like every other curation lever.
    HostTool {
        tool: "nav.ext_boards.get",
        group: "nav",
        description: "read the workspace's host-authored extension board rows",
    },
    HostTool {
        tool: "nav.ext_boards.set",
        group: "nav",
        description: "replace the workspace's host-authored extension board rows (admin)",
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
