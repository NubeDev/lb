//! `flows.debug.watch {flow_id}` + the per-flow debug publisher (debug-node-scope). The Node-RED
//! debug-node posture: a `debug` node publishes each wire message as **motion** onto a workspace-
//! walled subject, and a watcher (the canvas debug panel, or any MCP/bus listener) tails that stream
//! live. This is **motion only** (§3 rule 3): v1 holds NO durable record — a late-attaching watcher
//! sees messages from attach onward (deltas-only, no snapshot), and a message with no subscribers is
//! dropped (fire-and-forget). Persistence-to-disc is a named follow-up, not this scope.
//!
//! **Per-flow, not per-run** (Decision 3): the subject is `flow_debug:{ws}:{flow}` and the `run_id`
//! is attribution carried *in* each message, not the partition. This matches Node-RED's per-tab
//! sidebar and makes "open a flow, watch debug" honest for a triggered/source flow that has no
//! long-lived run for a browser to subscribe to.
//!
//! Workspace-walled two ways: `flows.debug.watch` runs the `mcp:flows.debug.watch:call` gate (opaque
//! deny), and the bus subject is prefixed `ws/{id}/` by `lb_bus` so a ws-B principal physically
//! cannot subscribe to a ws-A flow's debug stream (§7). The debug node itself needs no new cap to
//! publish — it runs inside a `flows.run` (already gated).

use lb_auth::Principal;
use lb_bus::{publish, subscribe, Bus, Subscription};
use lb_mcp::authorize_tool;
use serde_json::{json, Value};

use super::error::FlowsError;

/// The content-type a debug message carries — the author's declared `format` resolved at publish
/// time, so the panel renders deterministically (Decision 5). `auto` is resolved in [`resolve_format`]
/// before publish; the wire never carries `auto`.
pub const FORMAT_JSON: &str = "json";
pub const FORMAT_TEXT: &str = "text";
pub const FORMAT_MARKDOWN: &str = "markdown";

/// The workspace-relative subject a flow's debug stream rides on. `lb_bus` walls it under `ws/{id}/`
/// → `ws/{id}/flow/{flow_id}/debug`. The `flow/` prefix is host-internal (not a caller-nameable
/// `bus.*`/`ext/` subject), so it never collides with user subjects — the same prefix `flow_run_subject`
/// uses for the per-run settle feed.
pub fn flow_debug_subject(flow_id: &str) -> String {
    format!("flow/{flow_id}/debug")
}

/// Resolve a payload's content `format` at publish time (Decision 5). `auto` sniffs: a JSON
/// object/array value (or a string that parses as one) → `json`; a string carrying markdown markers
/// (a leading `#`/`-`/`*`/`>` or a fenced ``` ``` ```) → `markdown`; anything else → `text`. An
/// explicit `format` is authoritative and returned verbatim. This runs host-side so the browser is a
/// pure renderer and the publish/subscribe pair never disagrees on what a value is.
pub fn resolve_format(declared: &str, payload: &Value) -> &'static str {
    if declared == FORMAT_JSON {
        return FORMAT_JSON;
    }
    if declared == FORMAT_TEXT {
        return FORMAT_TEXT;
    }
    if declared == FORMAT_MARKDOWN {
        return FORMAT_MARKDOWN;
    }
    // `auto` (or any unknown declared value): sniff at publish time.
    // `auto` sniff. An object/array value is JSON by construction.
    if payload.is_object() || payload.is_array() {
        return FORMAT_JSON;
    }
    if let Value::String(s) = payload {
        // A string that parses as a JSON object/array → json (rendered as a tree).
        if serde_json::from_str::<Value>(s).map_or(false, |v| v.is_object() || v.is_array()) {
            return FORMAT_JSON;
        }
        if looks_like_markdown(s) {
            return FORMAT_MARKDOWN;
        }
    }
    FORMAT_TEXT
}

/// Heuristic: does this string read like markdown? A leading blockquote (`>`), a heading (`#`),
/// a bullet/checkbox (`-`/`*` at line start), or a fenced code block (``` ``` ```). Conservative —
/// the explicit `format` config is authoritative; `auto` only guesses.
fn looks_like_markdown(s: &str) -> bool {
    let trimmed = s.trim_start();
    if trimmed.starts_with("```") {
        return true;
    }
    for line in trimmed.lines().take(8) {
        let l = line.trim_start();
        if l.starts_with('#')
            && l.chars()
                .nth(1)
                .map_or(true, |c| c.is_whitespace() || c == '#')
        {
            return true;
        }
        if (l.starts_with("- ") || l.starts_with("* ")) && l.len() > 2 && !l.starts_with("- [")
        // not a checkbox variant we treat specially
        {
            return true;
        }
        if l.starts_with("> ") {
            return true;
        }
    }
    false
}

/// Publish one debug message for `flow_id`. Best-effort: a serialization or bus failure is dropped
/// (debug is fire-and-forget motion; no durable record to fall back to). The payload is the JSON the
/// SSE route forwards verbatim as one `debug` frame.
pub async fn publish_debug_event(bus: &Bus, ws: &str, flow_id: &str, event: &Value) {
    let Ok(bytes) = serde_json::to_vec(event) else {
        return;
    };
    let _ = publish(bus, ws, &flow_debug_subject(flow_id), &bytes).await;
}

/// Build a debug message — the unit the panel renders. Carries the attribution (`node`, `run_id`,
/// `ts`), the resolved `format`, the `value`, the author's `label` + `collapse_bytes` hint, and the
/// `format` the panel renders with. A `dropped` count replaces `value` when the publish governor
/// tripped (Risk 1) — built by [`dropped_event`], not this.
pub fn debug_message(
    node_id: &str,
    run_id: &str,
    ts: u64,
    format: &str,
    value: &Value,
    label: &str,
    collapse_bytes: u64,
) -> Value {
    json!({
        "kind": "debug",
        "node": node_id,
        "runId": run_id,
        "ts": ts,
        "format": format,
        "value": value,
        "label": label,
        "collapseBytes": collapse_bytes,
    })
}

/// A `dropped` sentinel — the publish governor tripped (Risk 1). The panel renders "N messages
/// dropped" rather than lagging silently. Carries the same attribution so it lands inline.
pub fn dropped_event(node_id: &str, run_id: &str, ts: u64, label: &str, dropped: u64) -> Value {
    json!({
        "kind": "dropped",
        "node": node_id,
        "runId": run_id,
        "ts": ts,
        "label": label,
        "dropped": dropped,
    })
}

/// A live subscription to one flow's debug subject, decoded to JSON events. Mirrors `FlowEventSub`.
pub struct DebugEventSub {
    inner: Subscription,
}

impl DebugEventSub {
    /// Await the next decoded debug event; skips an undecodable payload; `None` once closed.
    pub async fn recv(&self) -> Option<Value> {
        loop {
            let bytes = self.inner.recv().await?;
            match serde_json::from_slice::<Value>(&bytes) {
                Ok(event) => return Some(event),
                Err(_) => continue,
            }
        }
    }
}

/// What a watcher receives on attach. v1 is **deltas-only** (motion-only, Decision 2): there is no
/// snapshot — a late opener tails from now (Node-RED sidebar parity). Replay-on-open rides the
/// persistence follow-up (Open Q 2), not this struct.
pub struct FlowDebugWatch {
    /// The live delta feed (deltas-only in v1).
    pub stream: DebugEventSub,
}

/// `flows.debug.watch {flow_id}` — begin watching the debug stream for `flow_id` in workspace `ws`
/// as `principal`. Gated `mcp:flows.debug.watch:call` (opaque deny). The stream is deltas-only in v1;
/// a watcher that attaches after a message was published does NOT see it (no snapshot, no replay).
pub async fn watch_flow_debug(
    _store: &lb_store::Store,
    bus: &Bus,
    principal: &Principal,
    ws: &str,
    flow_id: &str,
) -> Result<FlowDebugWatch, FlowsError> {
    authorize_tool(principal, ws, "flows.debug.watch").map_err(|_| FlowsError::Denied)?;
    let inner = subscribe(bus, ws, &flow_debug_subject(flow_id))
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?;
    Ok(FlowDebugWatch {
        stream: DebugEventSub { inner },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_resolves_objects_arrays_and_json_strings_to_json() {
        assert_eq!(resolve_format("auto", &json!({"a": 1})), FORMAT_JSON);
        assert_eq!(resolve_format("auto", &json!([1, 2])), FORMAT_JSON);
        assert_eq!(resolve_format("auto", &json!("{\"x\":1}")), FORMAT_JSON);
        // A JSON scalar string does NOT promote to json (it's text-shaped, not a tree).
        assert_eq!(resolve_format("auto", &json!("42")), FORMAT_TEXT);
    }

    #[test]
    fn auto_resolves_markdown_marked_strings() {
        assert_eq!(
            resolve_format("auto", &json!("# Heading\nbody")),
            FORMAT_MARKDOWN
        );
        assert_eq!(
            resolve_format("auto", &json!("- item one\n- item two")),
            FORMAT_MARKDOWN
        );
        assert_eq!(resolve_format("auto", &json!("> quoted")), FORMAT_MARKDOWN);
        assert_eq!(
            resolve_format("auto", &json!("```rust\nfn main() {}\n```")),
            FORMAT_MARKDOWN
        );
    }

    #[test]
    fn auto_falls_back_to_text_for_plain_strings_and_scalars() {
        assert_eq!(resolve_format("auto", &json!("hello")), FORMAT_TEXT);
        assert_eq!(resolve_format("auto", &json!(23.4)), FORMAT_TEXT);
        assert_eq!(resolve_format("auto", &Value::Null), FORMAT_TEXT);
    }

    #[test]
    fn explicit_format_is_authoritative() {
        // An explicit `json` renders a plain string as a JSON tree node, by author intent.
        assert_eq!(resolve_format("json", &json!("hello")), FORMAT_JSON);
        assert_eq!(resolve_format("markdown", &json!("[1,2]")), FORMAT_MARKDOWN);
        assert_eq!(resolve_format("text", &json!({"a": 1})), FORMAT_TEXT);
        // An unknown declared value is treated as `auto`.
        assert_eq!(resolve_format("weird", &json!({"a": 1})), FORMAT_JSON);
    }

    #[test]
    fn a_dropped_sentinel_replaces_value_with_a_count() {
        let ev = dropped_event("d1", "r1", 10, "dbg", 7);
        assert_eq!(ev["kind"], "dropped");
        assert_eq!(ev["dropped"], 7);
        assert!(ev.get("value").is_none());
    }
}
