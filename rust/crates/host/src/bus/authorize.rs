//! The bus capability gate + the workspace-wall subject guard (widget-config-vars scope, "Platform
//! fix"). Two checks, both load-bearing:
//!
//!   1. **Capability** — `bus.publish`/`bus.watch` are host-native MCP verbs gated by `mcp:<verb>:call`
//!      through the shared `lb_mcp::authorize_tool` chokepoint (workspace-first, then capability). A
//!      denial is opaque [`BusError::Denied`].
//!   2. **Subject wall (rule 6)** — the caller's `subject` is a SUFFIX under the workspace wall: the
//!      host namespaces it as `ws/{id}/ext/{subject}` (the `ws/{id}/` prefix is added by `ws_key`, the
//!      `ext/` prefix here), so a caller can NEVER name another workspace's subject NOR platform motion
//!      (`series/`, `channels/`, internal). Reserved prefixes are rejected before any publish/subscribe.

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::BusError;

/// Prefixes a caller may NOT name — platform motion lives under these, so a `bus.*` subject naming one
/// would let a caller impersonate series/channel/internal traffic. Checked against the caller's raw
/// subject (before the `ext/` namespacing), case-sensitively.
const RESERVED: &[&str] = &["series/", "channels/", "internal/", "ws/", "presence/"];

/// Authorize the `<verb>` MCP surface (`bus.publish` / `bus.watch`) in workspace `ws`. Opaque deny.
pub fn authorize_bus(principal: &Principal, ws: &str, verb: &str) -> Result<(), BusError> {
    authorize_tool(principal, ws, verb).map_err(|_| BusError::Denied)
}

/// Validate a caller-supplied subject and return its workspace-RELATIVE key (`ext/{subject}`). The
/// `ws/{id}/` wall is added later by the bus layer's `ws_key`, so the returned key cannot escape the
/// workspace. Rejects an empty subject, a reserved prefix, and any `..`/absolute escape attempt.
pub fn wall_subject(subject: &str) -> Result<String, BusError> {
    let s = subject.trim();
    if s.is_empty() {
        return Err(BusError::BadSubject("empty subject".into()));
    }
    // No leading slash, no path traversal, no wall-escape — the subject is a plain suffix.
    if s.starts_with('/') || s.contains("..") {
        return Err(BusError::BadSubject(format!("illegal subject: {s}")));
    }
    let lower = s.to_ascii_lowercase();
    for r in RESERVED {
        if lower == r.trim_end_matches('/') || lower.starts_with(r) {
            return Err(BusError::BadSubject(format!("reserved prefix: {s}")));
        }
    }
    // Namespaced under `ext/` so a caller's subject can never collide with platform motion, even
    // before the `ws/{id}/` wall is prepended.
    Ok(format!("ext/{s}"))
}
