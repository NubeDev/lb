//! The coded refusals of the schedule verbs. Each message opens with a stable code a client matches
//! on; the rest is for people. Over MCP they arrive as `ToolError::BadInput`, which the gateway sends
//! as HTTP 400 with the body `bad input: <message>`.

use crate::flows::error::FlowsError;

/// A stored row has no `schedule` document: it was written by an lb before the rubix-ce shape.
pub const SCHEDULE_OLD_SHAPE: &str = "schedule_old_shape";

pub(super) fn old_shape(id: &str, rev: u64) -> FlowsError {
    FlowsError::BadInput(format!(
        "{SCHEDULE_OLD_SHAPE}: schedule `{id}` is in the old shape (no `schedule` document); save over it with rev {rev} or delete it"
    ))
}
