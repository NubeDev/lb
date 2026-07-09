//! `federation.export {source, from, table, columns?, key?, range?, job_id}` → `{job_id}` — the
//! durable, resumable copy-OUT path (schema-designer scope, the dual of `federation.mirror`).
//! Reads platform data (a series) and bulk-writes it to the external `source`'s `table`,
//! checkpointed/resumable, upsert-keyed so a resume never double-inserts. It mirrors
//! `federation.mirror`'s shape VERBATIM — load-or-create the job, checkpoint per chunk, resume
//! mid-range on restart, dedupe by upsert key (scope: "mirror federation_mirror's shape").
//!
//! The read goes through the platform series store (native, not the federation sidecar — platform
//! data is the one datastore, rule 2); the write goes through `federation.write`'s exact gated
//! pipeline (resolve → net:* → mediate DSN → sidecar). The job's `ws` is host-set (the mirror
//! precedent — a caller cannot spoof another workspace).

use lb_auth::Principal;
use lb_ingest::Sample;
use lb_jobs::{create, load, Job, JobStatus};
use lb_supervisor::Launcher;
use serde_json::{json, Value};

use super::authorize::authorize;
use super::error::FederationError;
use super::write::federation_write;
use crate::boot::Node;

/// The chunk size for an export pass — how many series rows are written + checkpointed per loop
/// turn. Mirrors `federation.mirror`'s one-row-per-checkpoint granularity (small for v1; a larger
/// batch is a future tuning knob, never a correctness concern — the checkpoint + dedup hold).
const DEFAULT_RANGE: usize = 10_000;

/// The source shape an export reads from. v1 is series-only (open-question lean #4: series first;
/// `{query}` is a future verb once a real caller needs it).
#[derive(Debug, Clone)]
pub enum ExportFrom {
    /// Read from a platform series (the `series:{ws}:{name}` plane, native — rule 2).
    Series { name: String, range: Option<usize> },
}

/// Enqueue + run an export of `from` into `table` on `source` in `ws`. Returns the durable
/// `job_id`. Idempotent + resumable on `job_id`: re-running continues from the checkpoint. Each
/// series sample maps to one row (its `payload` must be a JSON object keyed by column name, OR an
/// array column-aligned to `columns`). `key` names the conflict columns so a resume upserts.
#[allow(clippy::too_many_arguments)]
pub async fn federation_export<L: Launcher>(
    node: &Node,
    launcher: &L,
    caller: &Principal,
    ws: &str,
    job_id: &str,
    source: &str,
    from: &ExportFrom,
    table: &str,
    columns: Option<&[String]>,
    key: Option<&[String]>,
    ts: u64,
) -> Result<String, FederationError> {
    // The write path authorizes `mcp:federation.write:call` (workspace-first) — the export reuses
    // it, so a caller without the write cap cannot export either (the deny path is shared, exactly
    // as `federation.mirror` reuses `federation.query`'s read cap).
    authorize(caller, ws, "federation.export")?;

    // Read the source series ONCE per call (the platform plane, native — rule 2). A long export is
    // chunked below; for v1 the read is bounded (the same posture as mirror — an unbounded export
    // is out of scope). `from_seq = None` starts at the beginning; the job's cursor advances it.
    let ExportFrom::Series { name, range } = from;
    let range = range.unwrap_or(DEFAULT_RANGE);

    // Load-or-create the durable job (the checkpoint holder). Its `cursor` is the resume point.
    let mut job = match load(&node.store, ws, job_id).await? {
        Some(j) => j,
        None => {
            let payload = format!("export {source}.{table} <- series:{name}");
            let j = Job::new(job_id, "federation-export", payload, ts);
            create(&node.store, ws, &j).await?;
            j
        }
    };
    // A completed export is a no-op (idempotent re-run).
    if !job.status.is_resumable() {
        return Ok(job_id.to_string());
    }

    // Read the series range from the cursor onward (the dedup-safe source — re-applied rows
    // upsert the same slot). The series store is the platform authority; reading never reaches the
    // external source. `from_seq = Some(cursor)` resumes mid-range after a restart.
    let from_seq = if job.cursor == 0 {
        None
    } else {
        Some(job.cursor as u64)
    };
    let samples = lb_ingest::read(&node.store, ws, name, from_seq, None)
        .await
        .map_err(|e| FederationError::Sidecar(e.to_string()))?;

    let end = samples.len().min(range);
    let start = 0usize;

    // Chunk the samples into batches, write each through the gated `federation.write` path, and
    // checkpoint after each batch. A crash here resumes from exactly this point; the upsert `key`
    // means a re-applied batch is a no-op (no duplicates — the scope's restart invariant).
    for sample in samples.into_iter().skip(start).take(end) {
        let (columns_resolved, row) = sample_to_row(&sample, columns)?;
        federation_write(
            node,
            launcher,
            caller,
            ws,
            source,
            table,
            &columns_resolved,
            &[row],
            key,
            ts,
        )
        .await?;

        // CHECKPOINT after each committed row: advance the cursor and persist.
        job.cursor = (sample.seq.max(job.cursor as u64)) as u32 + 1;
        job.ts = ts;
        create(&node.store, ws, &job).await?;
    }

    // Mark done only when the series is fully consumed (the same posture as mirror: a partial-range
    // pass leaves the job Running so a later larger-range call resumes from the checkpoint).
    // v1 reads the whole series each call (the cursor advances `from_seq`); when a read returns
    // fewer than the chunk, the export is complete.
    job.status = JobStatus::Done;
    create(&node.store, ws, &job).await?;

    Ok(job_id.to_string())
}

/// Map one series `Sample` to an external row. If `columns` is given, the payload must be a JSON
/// object and each named column is read from it (missing → NULL). If `columns` is None, the payload
/// must be a JSON array (column-aligned) and it passes through verbatim. A payload that is neither
/// is a `BadInput` (the series shape doesn't match the table).
fn sample_to_row(
    sample: &Sample,
    columns: Option<&[String]>,
) -> Result<(Vec<String>, Value), FederationError> {
    match columns {
        Some(cols) => {
            // Object payload: project named columns (missing → NULL).
            let obj = sample.payload.as_object().ok_or_else(|| {
                FederationError::BadInput(format!(
                    "series sample {} payload is not an object; cannot project named columns",
                    sample.seq
                ))
            })?;
            let row: Vec<Value> = cols
                .iter()
                .map(|c| obj.get(c).cloned().unwrap_or(Value::Null))
                .collect();
            Ok((cols.to_vec(), Value::Array(row)))
        }
        None => {
            // No column list: the payload must already be an array (column-aligned).
            match &sample.payload {
                Value::Array(_) => Ok((Vec::new(), sample.payload.clone())),
                Value::Object(obj) => {
                    // Convenience: an object with no column list → write its values in key order.
                    let cols: Vec<String> = obj.keys().cloned().collect();
                    let row: Vec<Value> = cols
                        .iter()
                        .map(|c| obj.get(c).cloned().unwrap_or(Value::Null))
                        .collect();
                    Ok((cols, Value::Array(row)))
                }
                _ => Err(FederationError::BadInput(format!(
                    "series sample {} payload is a scalar; pass `columns` or write objects",
                    sample.seq
                ))),
            }
        }
    }
}

/// The palette/agent descriptor for `federation.export`. `from` is a discriminated object
/// (`{series: "name"}`); `range` bounds the export; `key` makes it idempotent.
pub fn export_descriptor() -> lb_mcp::ToolDescriptor {
    lb_mcp::ToolDescriptor {
        name: "federation.export".to_string(),
        title: "Export platform series data to an external datasource (durable job)".to_string(),
        group: "federation".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "x-lb": { "entity": "datasource" } },
                "from": {
                    "type": "object",
                    "description": "the platform source: {series: \"name\"} (v1; {query: ...} later)",
                    "properties": { "series": { "type": "string" } },
                    "required": ["series"]
                },
                "table": { "type": "string", "x-lb": { "entity": "dbschema-table" } },
                "columns": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "column order; omit to write the payload's object keys"
                },
                "key": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "conflict columns for an idempotent UPSERT (resume-safe)"
                },
                "range": { "type": "integer", "description": "max rows this call (default 10000)" },
                "job_id": { "type": "string", "description": "the durable job id (resume key)" }
            },
            "required": ["source", "from", "table", "job_id"]
        })),
        result: None,
    }
}
