//! The MCP bridge for ingest verbs — host-native tools under the one MCP contract (README §6.5).
//! UI, agents, and producers reach `ingest.write` / `series.read` / `series.latest` the SAME way
//! they reach any wasm tool: a qualified call with JSON in/out. The MCP gate (`authorize_ingest`)
//! runs inside each verb FIRST — a ws-B caller, or one without the grant, is refused before the
//! verb runs (the mandatory MCP-surface deny + isolation tests are real here).
//!
//! Host-native (not a wasm extension), so it is NOT in the runtime `Registry`; the gateway/UI route
//! `ingest.*` / `series.*` here.

use lb_auth::Principal;
use lb_ingest::Sample;
use lb_mcp::ToolError;
use lb_store::Store;
use lb_tags::Facet;
use serde_json::{json, Value};

use super::{
    drain_workspace_bounded, ingest_write_reporting, own_batches, series_latest_many,
    series_latest_value, IngestError,
};

/// Dispatch an ingest/series MCP call. `input` is the verb's JSON arguments; the return is the
/// verb's JSON result. Each verb authorizes first; denials are opaque (`ToolError::Denied`).
pub async fn call_ingest_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "ingest.write" => {
            let samples: Vec<Sample> = serde_json::from_value(arg(input, "samples")?.clone())
                .map_err(|e| ToolError::BadInput(format!("samples: {e}")))?;
            // Take the DIRECT path's filter counts with the acceptance: that path commits here, so
            // its counts exist nowhere else — the bounded drain below finds staging empty and
            // reports zeroes. On the stage path these are zero and the drain supplies them.
            let (n, direct_filtered) = ingest_write_reporting(store, principal, ws, samples)
                .await
                .map_err(ingest_error_to_tool)?;
            // Drain staging → the committed `series` table so the just-written sample is visible to
            // the very next `series.latest`/`read` over THIS same bridge — the round-trip the
            // proof-panel page proves; the gateway's own `POST /ingest` route drains for the same
            // reason. The drain is exactly-once per `(series, producer, seq)`, so a write-then-read
            // never double-commits.
            //
            // BOUNDED to the caller's own work (drain-backpressure scope): this used to drain until
            // staging was EMPTY, which billed the caller for every OTHER producer's staged rows —
            // one sample against a 4,671-row backlog measured 18.5s vs 21ms at backlog 0, and a
            // caller that timed out abandoned only the wait, so the backlog never drained and every
            // subsequent push blocked again. The bound is the caller's own sample count: enough to
            // commit what it just wrote (preserving the round-trip), never the workspace's backlog.
            // The background ingest reactor drains the remainder off every caller's path.
            let pass = drain_workspace_bounded(store, ws, own_batches(n))
                .await
                .map_err(ingest_error_to_tool)?;
            // `accepted` counts what reached STAGING — acceptance is deliberately unfiltered (the
            // filter is a commit-time decision, drain-backpressure scope). So a producer whose
            // samples the operator's own policy discards would otherwise see `accepted: 4` and find
            // two rows, with nothing on the wire explaining the gap. The per-reason counts of the
            // drain this call performed ride back with it — present only when something WAS
            // filtered, so every existing caller's reply shape is byte-for-byte unchanged.
            //
            // Bounded honesty: this drain commits oldest-first across the workspace, so on a node
            // with a backlog the counts may include other producers' rows. That is the same bound
            // `accepted` already lives under, and it is still the only view a writer gets.
            let mut filtered = direct_filtered;
            filtered.muted += pass.filtered.muted;
            filtered.range += pass.filtered.range;
            filtered.min_interval += pass.filtered.min_interval;
            filtered.deadband += pass.filtered.deadband;
            filtered.clamped += pass.filtered.clamped;
            let mut out = json!({ "accepted": n });
            if !filtered.is_zero() {
                out["filtered"] = json!(filtered);
            }
            Ok(out)
        }
        "series.read" => {
            let series = str_arg(input, "series")?;
            match input.get("mode").and_then(|v| v.as_str()).unwrap_or("rows") {
                "rows" => read_rows(store, principal, ws, series, input).await,
                "buckets" => read_buckets_mode(store, principal, ws, series, input).await,
                other => Err(ToolError::BadInput(format!("unknown mode: {other}"))),
            }
        }
        "series.rollup.read" => {
            let series = str_arg(input, "series")?;
            let from = u64_arg(input, "from")
                .ok_or_else(|| ToolError::BadInput("rollup read needs from (epoch ms)".into()))?;
            let to = u64_arg(input, "to")
                .ok_or_else(|| ToolError::BadInput("rollup read needs to (epoch ms)".into()))?;
            let rows = super::series_read_rollups(store, principal, ws, series, from, to)
                .await
                .map_err(ingest_error_to_tool)?;
            // The distinct tier widths present, ascending, alongside the rows. A caller plotting
            // stored rollups needs the grid it is plotting ON, and deriving it client-side means
            // every consumer re-implements the same scan. Empty rows → empty widths, which is the
            // honest shape for "nothing is stored here" — NOT an error, and never backfilled from
            // raw (see `series_read_rollups`).
            let mut widths: Vec<u64> = rows.iter().map(|r| r.width_ms).collect();
            widths.sort_unstable();
            widths.dedup();
            Ok(json!({ "rows": rows, "widths": widths }))
        }
        "series.retention.set" => {
            let policy: lb_ingest::Policy = serde_json::from_value(input.clone())
                .map_err(|e| ToolError::BadInput(format!("policy: {e}")))?;
            // `now_ms` caller-injectable (determinism §3); absent -> wall-clock, same as `.gc`.
            let now_ms = u64_arg(input, "now_ms").unwrap_or_else(now_wall_ms);
            let stored = super::series_retention_set(store, principal, ws, policy, now_ms)
                .await
                .map_err(ingest_error_to_tool)?;
            // The STORED policy, not `{ok:true}`. A set REPLACES the row, so returning what landed
            // is what lets a caller see a field their body omitted — the failure that started this.
            Ok(json!({ "ok": true, "policy": stored }))
        }
        "series.retention.patch" => {
            // Read-modify-write in ONE call: absent keys keep their stored value, and a supplied
            // tier is merged field-wise with the stored tier of the same width.
            let prefix = str_arg(input, "prefix")?;
            let now_ms = u64_arg(input, "now_ms").unwrap_or_else(now_wall_ms);
            let stored = super::series_retention_patch(store, principal, ws, prefix, input, now_ms)
                .await
                .map_err(ingest_error_to_tool)?;
            Ok(json!({ "ok": true, "policy": stored }))
        }
        "series.retention.list" => {
            let policies = super::series_retention_list(store, principal, ws)
                .await
                .map_err(ingest_error_to_tool)?;
            Ok(json!({ "policies": policies }))
        }
        "series.stats" => {
            let series = str_arg(input, "series")?;
            let stats = super::series_stats_get(store, principal, ws, series)
                .await
                .map_err(ingest_error_to_tool)?;
            Ok(json!(stats))
        }
        "series.retention.status" => {
            // ONE subject arg, which may be a full series id OR a bare prefix — longest-prefix
            // resolution is the same operation either way, so both callers share one code path.
            let series = str_arg(input, "series")?;
            let status = super::series_retention_status(store, principal, ws, series)
                .await
                .map_err(ingest_error_to_tool)?;
            Ok(json!(status))
        }
        "series.retention.delete" => {
            let prefix = str_arg(input, "prefix")?;
            super::series_retention_delete(store, principal, ws, prefix)
                .await
                .map_err(ingest_error_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "series.retention.gc" => {
            // `now_ms` is caller-injectable (determinism §3); absent → wall-clock.
            let now_ms = u64_arg(input, "now_ms").unwrap_or_else(now_wall_ms);
            let pass = super::series_retention_gc(store, principal, ws, now_ms)
                .await
                .map_err(ingest_error_to_tool)?;
            Ok(json!(pass))
        }
        "series.latest" => {
            let series = str_arg(input, "series")?;
            let last = series_latest_value(store, principal, ws, series)
                .await
                .map_err(ingest_error_to_tool)?;
            Ok(json!({ "sample": last }))
        }
        "series.latest_many" => {
            let names = string_arr(input, "series")?;
            let pairs = series_latest_many(store, principal, ws, &names)
                .await
                .map_err(ingest_error_to_tool)?;
            // `{ latest: { name: Sample|null } }` — every requested name present, absent → null, so
            // the caller reconciles nothing (parity with single series.latest's null contract).
            let latest: serde_json::Map<String, Value> = pairs
                .into_iter()
                .map(|(name, s)| (name, json!(s)))
                .collect();
            Ok(json!({ "latest": latest }))
        }
        "series.delete" => {
            let series = str_arg(input, "series")?;
            super::series_delete(store, principal, ws, series)
                .await
                .map_err(ingest_error_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "series.rename" => {
            let from = str_arg(input, "from")?;
            let to = str_arg(input, "to")?;
            super::series_rename(store, principal, ws, from, to)
                .await
                .map_err(ingest_error_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "series.find" => {
            let facets = facets(input)?;
            let hits = super::series_find(store, principal, ws, &facets)
                .await
                .map_err(ingest_error_to_tool)?;
            Ok(json!({ "series": hits }))
        }
        "series.list" => {
            // Prefix is optional — absent/empty lists every series.
            let prefix = input.get("prefix").and_then(|v| v.as_str()).unwrap_or("");
            let names = super::series_list(store, principal, ws, prefix)
                .await
                .map_err(ingest_error_to_tool)?;
            Ok(json!({ "series": names }))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// `series.read {mode:"rows"}` — the keyset page (paging scope, slice B). Legacy `from_seq`/`to_seq`
/// bounds still apply, joined by wall-clock `from`/`to` (epoch ms); the reply keeps the `samples`
/// key from the pre-paging wire shape and adds `next_cursor`/`prev_cursor`.
async fn read_rows(
    store: &Store,
    principal: &Principal,
    ws: &str,
    series: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    // Open bounds when omitted — never a `u64::MAX` sentinel (it coerces to a float and the
    // comparison mis-evaluates; see debugging/ingest/u64-max-bound-coerces-to-float.md).
    let q = lb_ingest::PageQuery {
        from_seq: u64_arg(input, "from_seq"),
        to_seq: u64_arg(input, "to_seq"),
        from_ts: u64_arg(input, "from"),
        to_ts: u64_arg(input, "to"),
        limit: u64_arg(input, "limit").map(|n| n as usize),
        cursor: input
            .get("cursor")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        direction: match input.get("direction").and_then(|v| v.as_str()) {
            Some("back") => lb_ingest::Direction::Back,
            _ => lb_ingest::Direction::Fwd,
        },
    };
    let page = super::series_read_page(store, principal, ws, series, &q)
        .await
        .map_err(ingest_error_to_tool)?;
    Ok(json!({
        "samples": page.rows,
        "next_cursor": page.next_cursor,
        "prev_cursor": page.prev_cursor,
    }))
}

/// `series.read {mode:"buckets"}` — server-side decimation (decimation scope, slice C). Requires a
/// wall-clock window `{from, to}` (epoch ms) and `width_ms` or `budget`.
///
/// `origin_ms` (optional, signed) pins where the buckets START, overriding the governing tier's own
/// alignment for this one read. Omit it — as every existing caller does — and the grid comes from the
/// policy, which is what makes a read agree with the way the GC folded (series-observability
/// Decision 21). It exists so a caller can PREVIEW a grid before committing it to a policy, and so a
/// chart can state its own day boundary without one.
async fn read_buckets_mode(
    store: &Store,
    principal: &Principal,
    ws: &str,
    series: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let q = lb_ingest::BucketQuery {
        from_ts: u64_arg(input, "from")
            .ok_or_else(|| ToolError::BadInput("buckets mode needs from (epoch ms)".into()))?,
        to_ts: u64_arg(input, "to")
            .ok_or_else(|| ToolError::BadInput("buckets mode needs to (epoch ms)".into()))?,
        width_ms: u64_arg(input, "width_ms"),
        budget: u64_arg(input, "budget").map(|n| n as usize),
        align: input
            .get("origin_ms")
            .filter(|v| !v.is_null())
            .map(|v| {
                v.as_i64()
                    .map(|origin_ms| lb_ingest::Align { origin_ms })
                    .ok_or_else(|| ToolError::BadInput("origin_ms must be an integer".into()))
            })
            .transpose()?,
    };
    let width = lb_ingest::effective_width(&q).map_err(ToolError::BadInput)?;
    // Optional per-read override of the tier's method. Absent → the governing tier decides; absent
    // there too → the full stat row and no `value` column, exactly as before this slice.
    let method = match input.get("method").and_then(|v| v.as_str()) {
        Some(name) => Some(lb_ingest::Method::parse(name).map_err(ToolError::BadInput)?),
        None => None,
    };
    let (buckets, resolved, align) =
        super::series_read_buckets(store, principal, ws, series, &q, width, method)
            .await
            .map_err(ingest_error_to_tool)?;
    // Report the method AND the grid back: a caller that relied on the tier's defaults should never
    // have to guess which method produced the `value` column it is charting, nor where the `t`
    // values it is plotting begin. `origin_ms` is `null` on the epoch grid — the same "absent means
    // today's behaviour" posture `method` already takes, so no existing caller sees a new number.
    Ok(json!({
        "buckets": buckets,
        "width_ms": width,
        "method": resolved.map(|m| m.as_str()),
        "origin_ms": align.map(|a| a.origin_ms),
    }))
}

/// Wall-clock now in epoch ms — ONLY the fallback for an omitted `series.retention.gc now_ms`.
fn now_wall_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Map the ingest gate's outcome onto the MCP tool error. `Denied` stays `Denied` (no existence
/// signal); a store/input error surfaces as `Extension`/`BadInput`.
/// Map the ingest service's error onto the MCP one. `pub(crate)` because `series.producer.health`
/// is dispatched from `tool_call` (it needs the node, not just the store) and must map its denial
/// through the SAME function — a second mapper is how "denied" comes to mean two things.
pub(crate) fn ingest_error_to_tool(e: IngestError) -> ToolError {
    match e {
        IngestError::Denied => ToolError::Denied,
        IngestError::BadInput(m) => ToolError::BadInput(m),
        IngestError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

fn arg<'a>(input: &'a Value, key: &str) -> Result<&'a Value, ToolError> {
    input
        .get(key)
        .ok_or_else(|| ToolError::BadInput(format!("missing arg: {key}")))
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    arg(input, key)?
        .as_str()
        .ok_or_else(|| ToolError::BadInput(format!("arg not a string: {key}")))
}

fn u64_arg(input: &Value, key: &str) -> Option<u64> {
    input.get(key).and_then(|v| v.as_u64())
}

/// Parse a required `[String]` argument (e.g. `series.latest_many`'s `series` name list).
fn string_arr(input: &Value, key: &str) -> Result<Vec<String>, ToolError> {
    let arr = arg(input, key)?
        .as_array()
        .ok_or_else(|| ToolError::BadInput(format!("arg not an array: {key}")))?;
    arr.iter()
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .ok_or_else(|| ToolError::BadInput(format!("{key}: entries must be strings")))
        })
        .collect()
}

/// Parse the `facets` array of a `series.find` query (value present → exact, absent → key-only).
fn facets(input: &Value) -> Result<Vec<Facet>, ToolError> {
    let arr = input
        .get("facets")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ToolError::BadInput("missing facets array".into()))?;
    arr.iter()
        .map(|f| {
            let key = f
                .get("key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::BadInput("facet missing key".into()))?;
            Ok(match f.get("value") {
                Some(v) => Facet::exact(key, v.clone()),
                None => Facet::key_only(key),
            })
        })
        .collect()
}
