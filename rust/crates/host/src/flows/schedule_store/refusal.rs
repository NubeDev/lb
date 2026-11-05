//! The coded refusals of the schedule verbs. Each message opens with a stable code a client matches
//! on; the rest is for people. Over MCP they arrive as `ToolError::BadInput`, which the gateway sends
//! as HTTP 400 with the body `bad input: <message>`.

use std::fmt::Display;

use crate::flows::error::FlowsError;

/// A stored row has no `schedule` document: it was written by an lb before the rubix-ce shape.
pub const SCHEDULE_OLD_SHAPE: &str = "schedule_old_shape";
/// The stored record is not at the `rev` the caller sent.
pub const SCHEDULE_STALE: &str = "schedule_stale";
/// A save without `rev` (a create) named an id that already exists.
pub const SCHEDULE_EXISTS: &str = "schedule_exists";
/// The save carried the removed top-level `weekly` / `exceptions`.
pub const SCHEDULE_LEGACY_FIELDS: &str = "schedule_legacy_fields";

pub(super) fn old_shape(id: &str, rev: u64) -> FlowsError {
    FlowsError::BadInput(format!(
        "{SCHEDULE_OLD_SHAPE}: schedule `{id}` is in the old shape (no `schedule` document); save over it with rev {rev} or delete it"
    ))
}

pub(super) fn stale(id: &str) -> FlowsError {
    FlowsError::BadInput(format!(
        "{SCHEDULE_STALE}: schedule `{id}` changed elsewhere, reload"
    ))
}

pub(super) fn exists(id: &str) -> FlowsError {
    FlowsError::BadInput(format!(
        "{SCHEDULE_EXISTS}: schedule `{id}` already exists; send its rev from schedule.get"
    ))
}

pub(super) fn legacy_fields(id: &str) -> FlowsError {
    FlowsError::BadInput(format!(
        "{SCHEDULE_LEGACY_FIELDS}: schedule `{id}` no longer takes `weekly` or `exceptions`; edit `schedule` instead"
    ))
}

/// A record that breaks a rule: `schedule `<id>`: <what>`.
pub(super) fn invalid(id: &str, what: impl Display) -> FlowsError {
    FlowsError::BadInput(format!("schedule `{id}`: {what}"))
}
