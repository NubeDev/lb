//! The `sink` node — a terminal write (flow-run-scope). `series`→`ingest.write`, `outbox`→the outbox
//! (must-deliver, idempotent), `channel`/`inbox`→`inbox.record`. The destination = `msg.topic ??
//! config.name` (the topic routes the message, like Node-RED); the sink writes `msg.payload`.

use std::sync::Arc;

use lb_auth::Principal;
use serde_json::{json, Value};

use crate::boot::Node;
use crate::tool_call::call_tool;

use super::super::run_store::NodeOutcome;
use super::tool_err_string;

#[allow(clippy::too_many_arguments)]
pub(super) async fn dispatch_sink(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    run_id: &str,
    node_id: &str,
    config: &Value,
    inputs: serde_json::Map<String, Value>,
    now: u64,
) -> NodeOutcome {
    let target = config.get("target").and_then(|v| v.as_str()).unwrap_or("");
    let config_name = config.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let topic = inputs.get("topic").and_then(|v| v.as_str());
    let name = topic.filter(|t| !t.is_empty()).unwrap_or(config_name);
    let value = inputs.get("payload").cloned().unwrap_or(Value::Null);
    match target {
        "series" => {
            // `ingest.write` needs the full `Sample` shape: `producer` present (stamped inside the
            // verb, send ""), `seq` = `now` (idempotent per firing), the point in `payload`.
            let req = json!({ "samples": [{
                "series": name,
                "producer": "",
                "ts": now,
                "seq": now,
                "payload": value.clone(),
            }] });
            match Box::pin(call_tool(
                node,
                principal,
                ws,
                "ingest.write",
                &req.to_string(),
            ))
            .await
            {
                Ok(_) => NodeOutcome::ok(json!({ "payload": value })),
                Err(e) => NodeOutcome::Err(tool_err_string(e)),
            }
        }
        "outbox" => {
            // A must-deliver sink stages an outbox effect (transactional; deterministic id → resume
            // no-op).
            let effect_id = format!("{run_id}:{node_id}");
            match crate::outbox::enqueue_outbox(
                &node.store,
                principal,
                ws,
                &effect_id,
                name,
                "write",
                &value.to_string(),
                now,
            )
            .await
            {
                Ok(()) => NodeOutcome::ok(json!({ "payload": value })),
                Err(e) => NodeOutcome::Err(e.to_string()),
            }
        }
        "channel" | "inbox" => {
            // `inbox.record` needs a stable `id` (idempotent on (channel,id)) — derive from (run,node);
            // `body` must be a STRING.
            let id = format!("{run_id}:{node_id}");
            let body = match &value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            let req = json!({ "channel": name, "id": id, "body": body, "ts": now });
            match Box::pin(call_tool(
                node,
                principal,
                ws,
                "inbox.record",
                &req.to_string(),
            ))
            .await
            {
                Ok(_) => NodeOutcome::ok(json!({ "payload": value })),
                Err(e) => NodeOutcome::Err(tool_err_string(e)),
            }
        }
        _ => NodeOutcome::Err(format!("sink node has unknown target: {target}")),
    }
}
