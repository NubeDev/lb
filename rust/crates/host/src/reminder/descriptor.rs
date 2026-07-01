//! The reminder **command-palette descriptors** (channel rich responses + reminders-tenant scope) —
//! `reminder.create`, `reminder.list`, `reminder.fire`. Declared in code beside the reminder verbs
//! (FILE-LAYOUT; a `descriptor.rs` collecting the reminder command descriptors, analogous to
//! `agent/descriptor.rs`); collected by `tools::host_descriptors`.
//!
//! Naming each descriptor EXACTLY after its verb is the load-bearing decision (same model as
//! `agent.invoke`): the catalog keeps a tool only if `authorize_tool(principal, ws, <name>)` passes,
//! so naming a descriptor `reminder.create` means the run's EXISTING `mcp:reminder.create:call` gate
//! decides catalog visibility with zero special-casing — a member who can create sees the command,
//! one who can't simply doesn't (absent, not greyed; no new cap, no `if` in the catalog).
//!
//! `create_descriptor`'s schema is **form-shaped**: it declares the flat fields the palette FORM
//! renders (a `cron` widget for `schedule`, a `select` for `action_kind`, then the per-kind action
//! fields as plain strings). The UI assembles the nested `action` object the `call_reminder_tool`
//! "create" branch expects BEFORE calling — so this descriptor never changes the verb contract.

use lb_mcp::ToolDescriptor;
use serde_json::{json, Value};

/// The form-shaped input schema for `reminder.create` — the flat fields the palette renders. `schedule`
/// drives the `cron` widget; `action_kind` drives a `select` over the three action kinds; the remaining
/// action fields are plain optional strings the form shows per the chosen kind. The verb's nested
/// `action` object is assembled UI-side from these before the call.
pub(crate) fn create_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "schedule": { "type": "string", "x-lb": { "widget": "cron" } },
            "action_kind": {
                "type": "string",
                "x-lb": { "widget": "select", "options": ["channel-post", "mcp-tool", "outbox"] }
            },
            "channel": { "type": "string" },
            "body": { "type": "string" },
            "tool": { "type": "string" },
            "args": { "type": "string" },
            "target": { "type": "string" },
            "action_action": { "type": "string" },
            "payload": { "type": "string" },
            "max_runs": { "type": "number" },
            "enabled": { "type": "boolean" }
        },
        "required": ["schedule", "action_kind"]
    })
}

/// The `reminder.create` descriptor — the "schedule a reminder" palette command. Gated on
/// `mcp:reminder.create:call` (the create verb's own gate) via the catalog's per-tool `authorize_tool`
/// — the name IS the gate.
pub fn create_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        name: "reminder.create".to_string(),
        title: "Schedule a reminder (cron + action)".to_string(),
        group: "reminder".to_string(),
        input_schema: Some(create_schema()),
        result: None,
    }
}

/// The interactive-table **response render envelope** `reminder.list` declares (`x-lb-render`). Mirrors
/// the `RichResultPayload` shape one-to-one (`v`/`view`/`source{tool,args}`/`options`/`action`/`tools`):
/// the palette POSTS this verbatim, the channel mounts it through the shipped `WidgetView`. `${id}` is
/// the ROW field the shipped vars engine interpolates (`${name}`, NOT `{{name}}`); `{{value}}` is the
/// switch's interaction value. `tools` is the declared bridge set the host intersects with the viewer's
/// grant. Declaring the render HERE is what keeps the frontend generic — it never hardcodes what
/// `reminder.list` renders as; it just posts what the descriptor carries.
pub(crate) fn list_render() -> Value {
    json!({
        "v": 2,
        "view": "table",
        "source": { "tool": "reminder.list", "args": {} },
        "options": { "rowControls": [
            { "kind": "switch", "label": "enabled",
              "action": { "tool": "reminder.update", "argsTemplate": { "id": "${id}", "enabled": "{{value}}" } } },
            { "kind": "button", "buttonLabel": "Run now",
              "action": { "tool": "reminder.fire", "argsTemplate": { "id": "${id}" } } },
            { "kind": "button", "buttonLabel": "Delete",
              "action": { "tool": "reminder.delete", "argsTemplate": { "id": "${id}" } } }
        ] },
        "tools": ["reminder.list", "reminder.update", "reminder.fire", "reminder.delete"]
    })
}

/// The `reminder.list` descriptor — the interactive "list reminders" palette command. An empty object
/// schema with optional `status`/`limit` (the same D3 list filter grammar the verb accepts). Gated on
/// `mcp:reminder.list:call` via the name.
pub fn list_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        name: "reminder.list".to_string(),
        title: "List reminders (interactive)".to_string(),
        group: "reminder".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "status": { "type": "string" },
                "limit": { "type": "number" }
            }
        })),
        // The interactive-table render this command's answer mounts as (the OUTPUT contract). The
        // palette posts this verbatim (interpolating collected args into `source.args`) instead of
        // showing a raw tool result — so the UI carries ZERO reminder-specific render knowledge.
        result: Some(list_render()),
    }
}

/// The `reminder.fire` descriptor — the "fire a reminder now" run-now control's command. Requires the
/// reminder `id`. Gated on `mcp:reminder.fire:call` via the name (so the run-now control's tool is in
/// the catalog only for a caller who may fire).
pub fn fire_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        name: "reminder.fire".to_string(),
        title: "Fire a reminder now".to_string(),
        group: "reminder".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "id": { "type": "string" }
            },
            "required": ["id"]
        })),
        result: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_schema_is_well_formed() {
        let s = create_schema();
        assert_eq!(s["type"], "object");
        // The cron widget hint drives the schedule editor.
        assert_eq!(s["properties"]["schedule"]["x-lb"]["widget"], "cron");
        // The select widget + the three action-kind options.
        assert_eq!(s["properties"]["action_kind"]["x-lb"]["widget"], "select");
        let opts = s["properties"]["action_kind"]["x-lb"]["options"]
            .as_array()
            .unwrap();
        assert!(opts.contains(&json!("channel-post")));
        assert!(opts.contains(&json!("mcp-tool")));
        assert!(opts.contains(&json!("outbox")));
        // required is exactly [schedule, action_kind].
        let required = s["required"].as_array().unwrap();
        assert_eq!(required.len(), 2);
        assert!(required.contains(&json!("schedule")));
        assert!(required.contains(&json!("action_kind")));
    }

    #[test]
    fn descriptors_are_named_after_their_verbs() {
        assert_eq!(create_descriptor().name, "reminder.create");
        assert_eq!(list_descriptor().name, "reminder.list");
        assert_eq!(fire_descriptor().name, "reminder.fire");
    }

    // The list command carries its response render (the OUTPUT contract): an interactive table over
    // `reminder.list`, with the three row controls (enabled switch, run-now, delete) bound to their
    // verbs. Agent B′ (UI) and Agent C assert against this exact shape — it is posted verbatim.
    #[test]
    fn list_descriptor_carries_the_interactive_table_render() {
        let render = list_descriptor().result.expect("list declares a render");
        assert_eq!(render["v"], 2);
        assert_eq!(render["view"], "table");
        assert_eq!(render["source"]["tool"], "reminder.list");
        assert!(render["source"]["args"].is_object());

        let controls = render["options"]["rowControls"].as_array().unwrap();
        assert_eq!(controls.len(), 3, "enabled switch + run-now + delete");
        // The switch toggles `enabled` via `reminder.update`, keyed on the row `${id}`.
        let switch = &controls[0];
        assert_eq!(switch["kind"], "switch");
        assert_eq!(switch["action"]["tool"], "reminder.update");
        assert_eq!(switch["action"]["argsTemplate"]["id"], "${id}");
        assert_eq!(switch["action"]["argsTemplate"]["enabled"], "{{value}}");
        // The two buttons fire and delete the row's reminder.
        assert_eq!(controls[1]["action"]["tool"], "reminder.fire");
        assert_eq!(controls[2]["action"]["tool"], "reminder.delete");

        // The declared bridge set is exactly the four verbs the render reaches.
        let tools = render["tools"].as_array().unwrap();
        for want in [
            "reminder.list",
            "reminder.update",
            "reminder.fire",
            "reminder.delete",
        ] {
            assert!(tools.contains(&json!(want)), "bridge declares {want}");
        }
    }

    #[test]
    fn fire_schema_requires_id() {
        let s = fire_descriptor().input_schema.unwrap();
        assert!(s["required"].as_array().unwrap().contains(&json!("id")));
    }
}
