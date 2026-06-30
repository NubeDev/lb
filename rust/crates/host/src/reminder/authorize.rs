//! The reminder capability gate — each `reminder.<verb>` is a host-native MCP tool, gated by
//! `mcp:reminder.<verb>:call` through the shared `lb_mcp::authorize_tool` chokepoint (workspace-
//! first, then capability). Same gate every MCP surface uses; the reminder verbs are not special.
//!
//! These gate the CRUD verbs (create / update / delete / get / list). They are independent of the
//! caps the firing re-checks *under the stored principal* — being allowed to create a reminder
//! never implies the action it fires (the firing re-runs the action's own gate at fire time).

use lb_auth::Principal;
use lb_mcp::authorize_tool;

use lb_reminders::ReminderError;

/// Authorize the `reminder.<verb>` MCP surface in workspace `ws` for `principal`. `Ok(())` only if
/// gate 1 (ws) and `mcp:reminder.<verb>:call` both pass. Any denial is opaque
/// [`ReminderError::Denied`].
pub fn authorize_reminder(
    principal: &Principal,
    ws: &str,
    verb: &str,
) -> Result<(), ReminderError> {
    let tool = format!("reminder.{verb}");
    authorize_tool(principal, ws, &tool).map_err(|_| ReminderError::Denied)
}
