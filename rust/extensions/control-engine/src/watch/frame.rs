//! The COV **frame contract** (slice-6 §"The frame contract"): re-encode `ce-client-rust`'s already
//! decoded `CovEvent` into ONE plumbing-agnostic JSON object per event. This is the real design work of
//! the slice — S7's wiresheet consumes exactly this shape off the SSE stream, and it must not care which
//! plumbing carries it. The sidecar NEVER re-implements the binary `wire.ts`: the client already decoded
//! the frame into typed `PropChange`/`TopologyEvent`, so this module only maps those into JSON.
//!
//! One responsibility per file: this file is the re-encode + its type rule, nothing else. The pump
//! (`super::pump`) writes these frames onto a series via the `ingest.write` host callback.
//!
//! Frame shapes:
//!   - `{ "kind": "cov", "ts": <ms>, "values": [{"uid","v"}...], "status": [{"uid","s"}...] }`
//!     — from `CovEvent::Values`. `status` carries only nonzero flags (a clean tick omits it).
//!   - `{ "kind": "topology", "ts": <ms>, "msg": {...} }` — from `CovEvent::Topology`.
//!
//! **The >2^53 rule (decided once, here):** JSON numbers cannot hold every `i64`/`u64` exactly past
//! `2^53`, and a float round-trip would silently corrupt an id-like integer. So an `i64` whose magnitude
//! exceeds `2^53` serializes as a JSON **string** (the wiresheet's `DecodedValue` handles the bigint).
//! Everything in the safe range stays a JSON number. `f64`/`bool`/`str`/`null` pass through as-is.
//!
//! **`schema` kind:** the pinned client's `CovEvent` has only `Values` + `Topology` — there is no schema
//! WS message surfaced. So we emit `cov` + `topology` only; when the client later surfaces a schema
//! message, add a `schema` arm here as a passthrough (tracked as a named follow-up in the session doc).

use rubix_ce::{CovEvent, FlexValue, PropChange, TopologyEvent, ValueFrame};
use serde_json::{json, Value};

/// The JS `Number.MAX_SAFE_INTEGER` boundary (`2^53 - 1`). An integer whose magnitude exceeds this
/// cannot survive a JSON-number/`f64` round-trip losslessly, so it is serialized as a string instead.
const MAX_SAFE_INT: i64 = 9_007_199_254_740_991; // 2^53 - 1

/// Re-encode one decoded [`CovEvent`] into its JSON frame. This is the whole contract.
#[must_use]
pub fn encode(event: &CovEvent) -> Value {
    match event {
        CovEvent::Values(frame) => encode_values(frame),
        CovEvent::Topology(topo) => encode_topology(topo),
    }
}

/// `CovEvent::Values` → the `cov` frame. `values` always present; `status` only for nonzero flags.
fn encode_values(frame: &ValueFrame) -> Value {
    let values: Vec<Value> = frame
        .changes
        .iter()
        .map(|c| json!({ "uid": c.uid, "v": encode_value(&c.value) }))
        .collect();
    let status: Vec<Value> = frame
        .changes
        .iter()
        .filter(|c| c.status_flags != 0)
        .map(|c: &PropChange| json!({ "uid": c.uid, "s": c.status_flags }))
        .collect();

    let mut out = json!({
        "kind": "cov",
        "ts": frame.timestamp_ms,
        "values": values,
    });
    if !status.is_empty() {
        out["status"] = Value::Array(status);
    }
    out
}

/// `CovEvent::Topology` → the `topology` frame. The engine sends a structural *signal* (the wiresheet
/// resyncs via `control-engine.tree`); we pass the decoded variant + its UIDs through as `msg`.
fn encode_topology(topo: &TopologyEvent) -> Value {
    let msg = match topo {
        TopologyEvent::Added {
            seq,
            component_uids,
            edge_uids,
        } => json!({
            "op": "added", "seq": seq,
            "componentUids": component_uids, "edgeUids": edge_uids,
        }),
        TopologyEvent::Removed {
            seq,
            component_uids,
            edge_uids,
        } => json!({
            "op": "removed", "seq": seq,
            "componentUids": component_uids, "edgeUids": edge_uids,
        }),
        TopologyEvent::Changed {
            seq,
            component_uids,
        } => json!({
            "op": "changed", "seq": seq,
            "componentUids": component_uids,
        }),
    };
    // Topology events carry no per-frame timestamp on the wire; use 0 (the wiresheet keys on `seq`).
    json!({ "kind": "topology", "ts": 0, "msg": msg })
}

/// Map a decoded [`FlexValue`] to its JSON form, applying the >2^53 → string rule to integers. `FlexValue`
/// is `#[serde(untagged)]`, so a float/bool/string/null already serializes to the natural JSON scalar;
/// only the `Int` arm needs the safety guard.
fn encode_value(v: &FlexValue) -> Value {
    match v {
        FlexValue::Int(n) if n.unsigned_abs() > MAX_SAFE_INT as u64 => Value::String(n.to_string()),
        other => serde_json::to_value(other).unwrap_or(Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rubix_ce::{PropChange, ValueFrame, MSG_UPDATE};

    fn change(uid: u32, value: FlexValue, status: u32) -> PropChange {
        PropChange {
            uid,
            value,
            status_flags: status,
        }
    }

    #[test]
    fn cov_frame_carries_uid_value_and_only_nonzero_status() {
        let frame = ValueFrame {
            msg_type: MSG_UPDATE,
            timestamp_ms: 171_234,
            changes: vec![
                change(1000100, FlexValue::Float(4.2), 0),
                change(1000101, FlexValue::Int(7), 3),
            ],
        };
        let out = encode(&CovEvent::Values(frame));
        assert_eq!(out["kind"], "cov");
        assert_eq!(out["ts"], 171_234);
        let values = out["values"].as_array().unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0]["uid"], 1000100);
        assert_eq!(values[0]["v"], 4.2);
        assert_eq!(values[1]["v"], 7);
        // Only the nonzero-status change appears in `status`.
        let status = out["status"].as_array().unwrap();
        assert_eq!(status.len(), 1);
        assert_eq!(status[0]["uid"], 1000101);
        assert_eq!(status[0]["s"], 3);
    }

    #[test]
    fn clean_tick_omits_the_status_array() {
        let frame = ValueFrame {
            msg_type: MSG_UPDATE,
            timestamp_ms: 1,
            changes: vec![change(1, FlexValue::Bool(true), 0)],
        };
        let out = encode(&CovEvent::Values(frame));
        assert!(
            out.get("status").is_none(),
            "no status key on a clean tick: {out}"
        );
        assert_eq!(out["values"][0]["v"], true);
    }

    #[test]
    fn big_integer_beyond_2_pow_53_serializes_as_string() {
        let big = MAX_SAFE_INT + 1; // 2^53 — the first unsafe value.
        let frame = ValueFrame {
            msg_type: MSG_UPDATE,
            timestamp_ms: 0,
            changes: vec![
                change(1, FlexValue::Int(big), 0),
                change(2, FlexValue::Int(-big), 0),
                change(3, FlexValue::Int(MAX_SAFE_INT), 0), // the boundary stays a number
            ],
        };
        let out = encode(&CovEvent::Values(frame));
        let values = out["values"].as_array().unwrap();
        assert_eq!(values[0]["v"], Value::String(big.to_string()));
        assert_eq!(values[1]["v"], Value::String((-big).to_string()));
        assert_eq!(
            values[2]["v"], MAX_SAFE_INT,
            "2^53-1 is still a JSON number"
        );
    }

    #[test]
    fn null_and_string_values_pass_through() {
        let frame = ValueFrame {
            msg_type: MSG_UPDATE,
            timestamp_ms: 0,
            changes: vec![
                change(1, FlexValue::Null, 0),
                change(2, FlexValue::Str("hi".into()), 0),
            ],
        };
        let out = encode(&CovEvent::Values(frame));
        assert_eq!(out["values"][0]["v"], Value::Null);
        assert_eq!(out["values"][1]["v"], "hi");
    }

    #[test]
    fn topology_added_passes_through_as_msg() {
        let ev = CovEvent::Topology(TopologyEvent::Added {
            seq: 9,
            component_uids: vec![10, 11],
            edge_uids: vec![20],
        });
        let out = encode(&ev);
        assert_eq!(out["kind"], "topology");
        assert_eq!(out["msg"]["op"], "added");
        assert_eq!(out["msg"]["seq"], 9);
        assert_eq!(out["msg"]["componentUids"][1], 11);
        assert_eq!(out["msg"]["edgeUids"][0], 20);
    }
}
