//! `viz.query(panel) -> { frames, rows }` — the backend panel-data resolver (viz transformations
//! scope). The host:
//!   1. authorizes `mcp:viz.query:call` (the verb gate — opaque deny);
//!   2. for EACH non-hidden target in the panel's `sources[]` (falling back to the v2 single
//!      `source`), dispatches the target tool by RE-ENTERING [`crate::call_tool_at_depth`] under the
//!      CALLER's principal + workspace — which re-checks that target tool's OWN cap and the workspace
//!      wall (no render-path bypass; a denied target degrades to an honest empty frame, never a
//!      fabricated value or a host-privilege read);
//!   3. assembles each target's rows into a canonical [`lb_viz::Frame`];
//!   4. runs the panel's `transformations[]` pipeline via `lb-viz` (the ONE transform impl);
//!   5. returns `{ frames }` (canonical, columnar) + `rows` (the primary frame flattened to the SAME
//!      row shape the shipped renderers consume — so the Phase-3 client swap changes nothing visible).
//!
//! The workspace comes from the TOKEN (the hard wall), never the panel spec. `lb-viz` is pure (no
//! store/bus) — the resolver is the only place store-backed data and the transform lib meet, over rows
//! already fetched through gated reads.

use std::sync::Arc;

use lb_auth::Principal;
use lb_viz::{transform, transform_stepwise, Frame, Frames, Transformation};
use serde_json::{json, Value};

use super::error::VizError;
use super::frame::{detect_time_field, result_to_rows};
use crate::boot::Node;
use crate::tool_call::call_tool_at_depth;

/// The per-panel frame budget (viz transformations scope, "The frame budget is the whole game"): a
/// single target is already capped (`store.query` 10k, `series.read` bounded, `federation.query`
/// row-capped); this bounds the ASSEMBLED set so a pathological multi-target panel can't blow up the
/// pipeline. An over-budget frame is truncated-with-note, never an unbounded crunch.
const MAX_ROWS_PER_FRAME: usize = 10_000;

/// Resolve a panel to canonical frames. `caller`/`ws` are the token's — every target dispatches under
/// them. `depth` is the resolver's own call depth (it re-enters dispatch at `depth + 1`). `now` is the
/// caller's logical clock, threaded into each target call (determinism; no wall-clock in core).
pub async fn viz_query(
    node: &Arc<Node>,
    caller: &Principal,
    ws: &str,
    panel: &Value,
    now: u64,
    depth: u32,
) -> Result<Value, VizError> {
    let pipeline = panel_pipeline(panel);

    // COMPUTE-ONLY (frames-in): the caller supplies raw `frames` inline and asks only for the transform
    // pipeline over them — no source resolution, no datasource touch. This is the editor "edit-without-
    // requery" path: a fieldConfig/transform tweak re-shapes frames ALREADY fetched instead of re-hitting
    // the datasource. It reaches NO gated read (it resolves nothing), so it stays purely inside the one
    // transform impl. Same verb, same `mcp:viz.query:call` gate — an inline `frames` just skips step 2.
    if let Some(frames) = panel_inline_frames(panel) {
        let frames = transform(frames, &pipeline);
        let rows: Vec<Value> = frames.first().map(Frame::to_rows).unwrap_or_default();
        return Ok(json!({ "frames": frames, "rows": rows }));
    }

    let targets = panel_targets(panel)?;

    // Dispatch each non-hidden target under the caller's authority; a denied/failed target → an
    // honest EMPTY frame (no bypass, no fabricated rows). Frames keep target order so refIds line up.
    let mut frames: Frames = Vec::with_capacity(targets.len());
    for t in &targets {
        let rows = dispatch_target(node, caller, ws, t, now, depth).await;
        let rows = cap_rows(rows);
        let time = detect_time_field(&rows);
        frames.push(Frame::from_rows(&t.ref_id, &rows, time.as_deref()));
    }

    // DEBUG (editor-parity step 7): when the panel asks for per-step frames, run the pipeline stepwise
    // and return a `steps[]` snapshot (input + one per applied step), honoring an optional `stopAt`.
    // Additive + opt-in — inherits the same `mcp:viz.query:call` cap, no new verb. The same frame budget
    // applies (it's the same `apply_step`), so the debug view can't blow past the cap either.
    if let Some(debug) = panel_debug(panel) {
        let snapshots = transform_stepwise(frames, &pipeline, debug.stop_at);
        // The last snapshot's primary frame flattened to rows = the effective result (so the preview
        // still renders while the debug view is on). Computed BEFORE serializing the snapshots.
        let rows: Vec<Value> = snapshots
            .last()
            .and_then(|(_, f)| f.first())
            .map(Frame::to_rows)
            .unwrap_or_default();
        // Cap each snapshot's frames to the per-frame budget (defense in depth) and emit.
        let steps: Vec<Value> = snapshots
            .into_iter()
            .map(|(step, frames)| json!({ "step": step, "frames": cap_frames(frames) }))
            .collect();
        return Ok(json!({ "steps": steps, "rows": rows }));
    }

    // Run the transform pipeline — the ONE impl, server-side (invariant B).
    let frames = transform(frames, &pipeline);

    // The primary frame (first) flattened to rows = the SAME shape the shipped client fetch produced,
    // so renderers/preview are unchanged. Empty when there is no frame.
    let rows: Vec<Value> = frames.first().map(Frame::to_rows).unwrap_or_default();

    Ok(json!({ "frames": frames, "rows": rows }))
}

/// One resolved target (the bits the resolver needs — its refId, tool, and args).
struct ResolvedTarget {
    ref_id: String,
    tool: String,
    args: Value,
}

/// The panel's targets, v3 — `sources[]` (non-hidden) when present, else the v2 single `source` as a
/// one-element `[A]`, else `[]`. Mirrors the client `cellSources`/`cellPrimaryTarget` adapter so the
/// host and client agree on which targets a panel has. A target with an empty tool is skipped (an
/// unconfigured slot is not a denied call).
fn panel_targets(panel: &Value) -> Result<Vec<ResolvedTarget>, VizError> {
    let mut out = Vec::new();
    if let Some(Value::Array(sources)) = panel.get("sources") {
        for s in sources {
            if s.get("hide").and_then(Value::as_bool).unwrap_or(false) {
                continue;
            }
            let tool = s.get("tool").and_then(Value::as_str).unwrap_or("");
            if tool.is_empty() {
                continue;
            }
            out.push(ResolvedTarget {
                ref_id: s
                    .get("refId")
                    .and_then(Value::as_str)
                    .unwrap_or("A")
                    .to_string(),
                tool: tool.to_string(),
                args: s.get("args").cloned().unwrap_or(json!({})),
            });
        }
    }
    if out.is_empty() {
        // v2 fallback: the single `source { tool, args }`.
        if let Some(src) = panel.get("source") {
            let tool = src.get("tool").and_then(Value::as_str).unwrap_or("");
            if !tool.is_empty() {
                out.push(ResolvedTarget {
                    ref_id: "A".into(),
                    tool: tool.to_string(),
                    args: src.get("args").cloned().unwrap_or(json!({})),
                });
            }
        }
    }
    Ok(out)
}

/// The caller-supplied inline `frames` for the compute-only (frames-in) path, if present. When the
/// panel carries a `frames` array we run the pipeline over THOSE and resolve no sources — the editor's
/// "shape without re-fetch" mode. A malformed frame is dropped; an absent/empty/non-array `frames`
/// returns `None` so the normal source-resolving path runs. Each frame is truncated to the per-frame
/// budget (defense in depth: a caller can't post an unbounded frame to crunch the pipeline).
fn panel_inline_frames(panel: &Value) -> Option<Frames> {
    let Some(Value::Array(arr)) = panel.get("frames") else {
        return None;
    };
    if arr.is_empty() {
        return None;
    }
    let frames: Frames = arr
        .iter()
        .filter_map(|f| serde_json::from_value::<Frame>(f.clone()).ok())
        .map(|mut f| {
            f.truncate(MAX_ROWS_PER_FRAME);
            f
        })
        .collect();
    if frames.is_empty() {
        None
    } else {
        Some(frames)
    }
}

/// The panel's transformation pipeline (opaque config on the cell → typed `Transformation[]`). A
/// malformed entry is skipped (preserved-but-not-run is the pipeline's job; a non-array is empty).
fn panel_pipeline(panel: &Value) -> Vec<Transformation> {
    match panel.get("transformations") {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|t| serde_json::from_value::<Transformation>(t.clone()).ok())
            .collect(),
        _ => Vec::new(),
    }
}

/// Dispatch ONE target by re-entering the host's generic MCP dispatcher under the caller's authority.
/// This composes the target tool's OWN cap check + the workspace wall + the exact store.query/
/// series.*/federation.query routing — no re-implemented dispatch, no privilege escalation. A denial
/// or any tool error → an EMPTY row set (honest empty frame; the deny is opaque and never a fabricated
/// value). `now` is threaded so a target verb that needs a logical clock (federation) gets one.
async fn dispatch_target(
    node: &Arc<Node>,
    caller: &Principal,
    ws: &str,
    t: &ResolvedTarget,
    now: u64,
    depth: u32,
) -> Vec<Value> {
    // Thread the caller's logical `now` into the args (a federation/ingest verb reads `ts` from args;
    // a store.query ignores it). Never overwrite a caller-supplied ts.
    let mut args = t.args.clone();
    if let Value::Object(map) = &mut args {
        map.entry("ts").or_insert(json!(now));
    }
    let input = args.to_string();

    // Box the recursive future: this re-enters the dispatcher, which can route back to `viz.query`
    // (a viz target of a viz panel) — a static async cycle Rust requires boxing to size.
    let dispatched = Box::pin(call_tool_at_depth(
        node,
        caller,
        ws,
        &t.tool,
        &input,
        depth + 1,
    ))
    .await;
    match dispatched {
        Ok(out) => serde_json::from_str::<Value>(&out)
            .map(|v| result_to_rows(&v))
            .unwrap_or_default(),
        // Denied / NotFound / any tool error → honest empty (no bypass, no fabrication).
        Err(_) => Vec::new(),
    }
}

/// Enforce the per-frame budget — truncate an over-cap target result rather than feed the pipeline an
/// unbounded frame (the resolver's load-bearing bound).
fn cap_rows(mut rows: Vec<Value>) -> Vec<Value> {
    if rows.len() > MAX_ROWS_PER_FRAME {
        rows.truncate(MAX_ROWS_PER_FRAME);
    }
    rows
}

/// The per-step debug request on the panel (editor-parity step 7). `panel.debug = true` (or an object
/// `{ stopAt?: number }`) turns on the stepwise view. Absent/false → no debug. `stopAt` bounds the
/// number of APPLIED steps to run.
struct DebugRequest {
    stop_at: Option<usize>,
}

fn panel_debug(panel: &Value) -> Option<DebugRequest> {
    match panel.get("debug") {
        Some(Value::Bool(true)) => Some(DebugRequest { stop_at: None }),
        Some(Value::Object(o)) => Some(DebugRequest {
            stop_at: o.get("stopAt").and_then(Value::as_u64).map(|n| n as usize),
        }),
        _ => None,
    }
}

/// Cap each frame in a debug snapshot to the per-frame row budget (defense in depth: a stepwise view
/// must not emit an unbounded intermediate frame). Re-serializes the capped frames to `Value`.
fn cap_frames(frames: Frames) -> Vec<Value> {
    frames
        .into_iter()
        .map(|mut f| {
            f.truncate(MAX_ROWS_PER_FRAME);
            serde_json::to_value(f).unwrap_or(Value::Null)
        })
        .collect()
}
