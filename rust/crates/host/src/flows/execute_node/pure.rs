//! The **pure** Tier-A data/JSON pack dispatch: a thin wrapper mapping a node type to one
//! `lb_flows::ops` function (data-nodes scope). No store, no bus, no tool dispatch — the op is a pure
//! function of the node's `config` + incoming `payload`, and its `Ok` value becomes the emitted
//! `payload`. A parse failure (`csv`/`xml`/`yaml`/`base64` on malformed input) returns `Err`, failing
//! the node under the flow's `FailurePolicy` — the `json`-node parity.
//!
//! `split` is the one shape that emits more than `payload` (it stamps the `parts` sequence descriptor,
//! Decision 15), so it returns the whole emitted envelope map; `join` reads the whole incoming message
//! (it needs the carried `parts` to recombine). Everything else is `payload → payload`.

use lb_flows::ops;
use serde_json::{json, Value};

use super::super::run_store::NodeOutcome;

/// Dispatch a pure data/JSON pack node. Returns `None` when `node_type` is not one of these (the
/// caller then tries the stateful/engine/spine legs).
pub(super) fn dispatch_pure(
    node_type: &str,
    config: &Value,
    inputs: &serde_json::Map<String, Value>,
) -> Option<NodeOutcome> {
    let payload = inputs.get("payload").cloned().unwrap_or(Value::Null);
    // The mode-carrying parse nodes read `config.mode`; the default matches the descriptor default.
    let mode = config.get("mode").and_then(|v| v.as_str());
    let out: Result<Value, String> = match node_type {
        // Data category.
        "change" => ops::data::change(config, &payload),
        "select" => ops::data::select(config, &payload),
        "merge" => ops::data::merge(&payload),
        "map" => ops::data::map(config, &payload),
        "flatten" => ops::data::flatten(config, &payload),
        "sort" => ops::data::sort(config, &payload),
        "range" => ops::data::range(config, &payload),
        "aggregate" => ops::data::aggregate(config, &payload),
        "template" => ops::template::render(config, &payload),
        // Parse category (malformed → Err → node fails).
        "csv" => ops::parse::csv(config, &payload, mode.unwrap_or("parse")),
        "xml" => ops::parse::xml(config, &payload, mode.unwrap_or("parse")),
        "yaml" => ops::parse::yaml(config, &payload, mode.unwrap_or("parse")),
        "base64" => ops::parse::base64(config, &payload, mode.unwrap_or("encode")),
        // Sequence: `split` emits payload + `parts`; `join` reads the whole message.
        "split" => {
            return Some(match ops::sequence::split(config, &payload) {
                Ok(map) => NodeOutcome::ok(Value::Object(map)),
                Err(e) => NodeOutcome::Err(e),
            })
        }
        "join" => ops::sequence::join(inputs),
        _ => return None,
    };
    Some(match out {
        Ok(v) => NodeOutcome::ok(json!({ "payload": v })),
        Err(e) => NodeOutcome::Err(format!("{node_type}: {e}")),
    })
}
