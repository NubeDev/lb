//! The **schedule** built-in descriptor — a time-driven source node over the workspace's *global*
//! schedule records. Sibling to [`super::core`] / [`super::observability`] / [`super::platform`], in
//! the one shared [`NodeDescriptor`] shape.
//!
//! The node is a [`NodeKind::Trigger`]: no inputs, host-fired. It holds a **`schedule_id` reference**,
//! never the schedule data itself — that lives in one workspace-scoped `schedule` record, so a single
//! "Building Hours" is shared by every node and widget that names it and is edited in exactly one
//! place (the global-schedules requirement). Swapping which schedule a node follows is a config edit,
//! not a re-authoring of its windows.
//!
//! Firing is **edge-triggered**: the reactor evaluates the referenced schedule on its own durable
//! cursor and fires only when the active state *changes* (inactive→active or active→inactive), so a
//! flow reacts to transitions rather than being re-run every tick. `emit_interval` opts into an
//! additional heartbeat for downstream consumers that want a periodic restatement of the current
//! state (the Go node's ticker behaviour) without changing the transition semantics.
//!
//! Ports speak the message envelope (D6): `payload` carries the boolean active state, `topic` the
//! schedule id. The richer detail (which source won, next transition) rides the envelope's payload
//! object so no extra wired ports are needed.

use serde_json::json;

use crate::descriptor::{NodeDescriptor, NodeKind};

/// The default re-evaluation cadence (seconds) for a schedule node's cursor. Ten seconds matches the
/// Go node's default and is far finer than any minute-resolution schedule needs.
pub const DEFAULT_EVALUATION_INTERVAL: u64 = 10;

/// The schedule pack: the single `schedule` source node.
pub fn schedule_descriptors() -> Vec<NodeDescriptor> {
    vec![
        // A time SOURCE over a global schedule record. No inputs; envelope out. The reactor owns the
        // clock + the last-known active state in one durable cursor, and fires this node's subgraph on
        // a transition. Empty `tool` — host-resolved, like `trigger`/`flipflop` (no MCP dispatch).
        NodeDescriptor::new("schedule", NodeKind::Trigger, "")
            .with_title("Schedule")
            .with_category("Flow")
            .with_icon("calendar-clock")
            .with_ports(vec![], vec!["payload".into(), "topic".into()])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "required": ["schedule_id"],
                    "additionalProperties": false,
                    "properties": {
                        "schedule_id": {
                            "type": "string",
                            // The editor renders this as the workspace's schedule roster picker
                            // (`schedule.list`), not a bare text box. An opaque format hint — the UI
                            // resolves it generically, the host never branches on a node id (rule 10).
                            "format": "lb:schedule",
                            "title": "Schedule",
                            "description": "The global schedule this node follows (schedule.list). The windows live on that shared record — edit them once and every node and widget referencing it follows."
                        },
                        "evaluation_interval": {
                            "type": "integer",
                            "minimum": 1,
                            "default": DEFAULT_EVALUATION_INTERVAL,
                            "title": "Evaluation interval (seconds)",
                            "description": "How often the schedule is re-evaluated to detect a transition. This is the detection resolution, not the firing rate."
                        },
                        "emit_interval": {
                            "type": "boolean",
                            "default": false,
                            "title": "Emit every interval",
                            "description": "Also fire on every evaluation, not only on a change of state — a heartbeat restating the current value. Off by default: transitions alone keep runs proportional to real schedule changes."
                        },
                        "invert": {
                            "type": "boolean",
                            "default": false,
                            "title": "Invert",
                            "description": "Emit `true` while the schedule is INACTIVE. Useful for out-of-hours branches without authoring a mirrored schedule."
                        },
                        "topic": {
                            "type": "string",
                            "title": "Topic",
                            "description": "Overrides the topic stamped on the firing envelope (defaults to the schedule id)."
                        }
                    }
                }),
            ),
    ]
}
