//! The core surface → gate-cap map (nav scope, the `surface` cap-strip). `nav.resolve` strips a
//! `surface` item the caller can't reach; a surface is "reachable" iff the caller holds the cap that
//! gates its page — the SAME cap the UI's `allowedSurfaces` checks, so the rendered rail and the
//! resolved payload agree. This is the backend mirror of `ui/src/features/routing/allowed.ts`.
//!
//! Surfaces are **core** (channels, rules, flows, …), so a core table keyed by surface is allowed —
//! rule 10 forbids branching on an **extension** id, not on a core surface. An `ext` item is handled
//! separately (via the opaque `ext.list` seam), never here. A surface not in this table is treated as
//! **always allowed** (like the UI's unconditional `channels/inbox/outbox/settings`) — a new surface
//! is visible by default and only gated once its cap is added here, matching the UI's default-visible
//! set. Keep this list in lockstep with `allowedSurfaces` (the scope's "fallback correctness" risk).

/// The cap that gates each core surface's page, or `None` for the always-visible surfaces (every
/// member reaches them). Returns the cap string a caller must hold for `surface` to survive the strip.
pub fn surface_gate_cap(surface: &str) -> Option<&'static str> {
    match surface {
        // Always visible — every member reaches these (mirrors `allowedSurfaces`' seed set).
        "channels" | "inbox" | "outbox" | "settings" => None,
        // Cap-gated core pages (the cap `allowedSurfaces` checks for each).
        "dashboards" => Some("mcp:dashboard.list:call"),
        "rules" => Some("mcp:rules.run:call"),
        "flows" => Some("mcp:flows.list:call"),
        "datasources" => Some("mcp:datasource.list:call"),
        "reminders" => Some("mcp:reminder.list:call"),
        "ingest" => Some("mcp:series.list:call"),
        "data" => Some("mcp:store.scan:call"),
        "system" => Some("mcp:system.overview:call"),
        "system-mcp" => Some("mcp:system.tools:call"),
        "system-acp" => Some("mcp:system.acp:call"),
        "telemetry" => Some("mcp:telemetry.read:call"),
        "extensions" => Some("mcp:ext.list:call"),
        "studio" => Some("mcp:devkit.templates:call"),
        // `admin` is gated by the admin role (any admin cap); the resolver checks a representative
        // admin cap. A caller without it never sees the admin surface in a shared nav.
        "admin" => Some("mcp:grants.assign:call"),
        // Unknown/new surface — visible by default (default-allow, like the UI seed set), gated only
        // once its cap is added above.
        _ => None,
    }
}
