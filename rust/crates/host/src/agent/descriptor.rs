//! The `agent.invoke` `tools.catalog` descriptor (external-agent run-lifecycle #5, the composer
//! runtime picker). Declared in code beside the agent verbs (FILE-LAYOUT); collected by
//! `tools::host_descriptors`. Naming the palette command **`agent.invoke`** is the load-bearing
//! decision:
//!
//! - The catalog keeps a tool only if `authorize_tool(principal, ws, <name>)` passes. Because the run
//!   is already gated by `mcp:agent.invoke:call`, naming the descriptor `agent.invoke` means THAT
//!   EXISTING gate decides catalog visibility with **zero special-casing** — a member who can run the
//!   agent sees the `/agent.invoke` command; one who can't simply doesn't (the catalog's "absent, not
//!   greyed" model — no new `agent.<x>:call` cap, no `if` in the catalog, no existence leak).
//! - The palette routes this descriptor to `postAgent` (the `kind:"agent"` payload path), NOT to a raw
//!   `agent.invoke` tool call — the descriptor is the menu entry + arg rail, the run still flows through
//!   the channel agent worker. See `CommandPalette.tsx`.
//!
//! The `runtime` arg carries `x-lb:{widget:"runtime"}` — the UI renders a `RuntimeArg` dropdown fed by
//! the `agent.runtimes` read verb (default preselected), replacing the old typed `@id`.

use lb_mcp::ToolDescriptor;
use serde_json::{json, Value};

/// The canonical input schema for the agent command — `{ goal, runtime }`. `goal` is required
/// free-text; `runtime` is optional and drives the `x-lb:{widget:"runtime"}` dropdown (fed by
/// `agent.runtimes`, defaulting to `default`).
pub(crate) fn invoke_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "goal": { "type": "string" },
            "runtime": { "type": "string", "x-lb": { "widget": "runtime" } }
        },
        "required": ["goal"]
    })
}

/// The `agent.invoke` descriptor — the in-channel agent as a first-class palette command. Its
/// visibility is gated by `mcp:agent.invoke:call` (the run's own gate) via the catalog's per-tool
/// `authorize_tool`; see the module docs for why the name IS the gate.
pub fn invoke_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "agent.invoke".to_string(),
        title: "Ask the in-channel agent to pursue a goal (pick a runtime)".to_string(),
        group: "agent".to_string(),
        input_schema: Some(invoke_schema()),
        result: None,
    }
}
