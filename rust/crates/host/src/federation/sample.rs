//! `federation.sample {source, tables?, limit?}` → one AI-ready snapshot of a registered source
//! (datasource-samples scope): every table's columns, its foreign keys (best-effort, per-kind),
//! and up to `limit` (default 10, cap 50) real rows — the context a model needs to write correct
//! SQL in ONE round trip instead of N+1 `federation.schema` calls with no relationship metadata.
//!
//! It reuses `federation.schema`'s exact gated pipeline — resolve the source IN THE CALLER'S
//! WORKSPACE, enforce `net:*`, mediate the DSN, ONE supervised-sidecar call — and authorizes under
//! the SAME read cap (`mcp:federation.query:call`): sampling is the same read privilege as a live
//! query, so no new capability/grant is introduced. Bounding, cell truncation, and the fixed
//! sensitive-column redaction live sidecar-side (`extensions/federation/src/sample.rs`).

use lb_auth::Principal;
use lb_supervisor::Launcher;
use serde_json::{json, Value};

use super::authorize::authorize;
use super::error::FederationError;
use super::net::{enforce_endpoint, FEDERATION_EXT};
use super::record::resolve;
use super::secret::mediate_dsn;
use crate::boot::Node;

/// The host-side clamp on sample rows per table (the sidecar clamps again — defense in depth).
const MAX_ROWS: u64 = 50;
const DEFAULT_ROWS: u64 = 10;

/// Snapshot `source` in `ws` as `caller`. `tables` filters to the named tables when present;
/// `limit` rows per table, clamped to 1..=50 (default 10). Returns the sidecar's JSON snapshot
/// (`{tables:[…], relationships:[…], truncated}`). The DSN is mediated host-side, never returned.
pub async fn federation_sample<L: Launcher>(
    node: &Node,
    launcher: &L,
    caller: &Principal,
    ws: &str,
    source: &str,
    tables: Option<&[String]>,
    limit: Option<u64>,
    ts: u64,
) -> Result<Value, FederationError> {
    // Sampling is the same read privilege as a live query — authorize under the read cap so no new
    // capability grant is needed (same decision as `federation.schema`).
    authorize(caller, ws, "federation.query")?;

    // Resolve the alias to a registered source IN THIS workspace — un-spoofable (the wall).
    let ds = resolve(&node.store, ws, source)
        .await?
        .ok_or(FederationError::NotFound)?;

    // `net:*` — refuse, opaque, if the source's endpoint is not in the admin-approved grant.
    enforce_endpoint(&node.store, ws, &ds.endpoint).await?;

    // Mediate the DSN under the FEDERATION extension's own grant (never the caller's).
    let dsn = mediate_dsn(node, ws, &ds.secret_ref).await?;

    let limit = limit.unwrap_or(DEFAULT_ROWS).clamp(1, MAX_ROWS);
    let mut input = json!({ "kind": ds.kind, "dsn": dsn, "source": source, "limit": limit });
    if let Some(tables) = tables {
        input["tables"] = json!(tables);
    }
    let input = input.to_string();

    let out = crate::native::call_sidecar(
        node,
        launcher,
        caller,
        ws,
        FEDERATION_EXT,
        "federation.sample",
        &input,
        ts,
    )
    .await
    .map_err(|e| FederationError::Sidecar(e.to_string()))?;

    let mut value: Value =
        serde_json::from_str(&out).map_err(|e| FederationError::Sidecar(e.to_string()))?;
    value["source"] = json!(source);
    Ok(value)
}

/// The palette/agent descriptor for `federation.sample` — a real arg schema so a model advertised
/// the tool can FORM a valid call (the `federation.schema` lesson: a name-only row leaves it
/// guessing arg names). `x-lb entity: datasource` drives the same `@`-picker as its siblings.
pub fn sample_descriptor() -> lb_mcp::ToolDescriptor {
    lb_mcp::ToolDescriptor {
        name: "federation.sample".to_string(),
        title: "Snapshot a datasource for AI: tables, columns, foreign keys, sample rows"
            .to_string(),
        group: "federation".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "x-lb": { "entity": "datasource" } },
                "tables": { "type": "array" },
                "limit": { "type": "integer" }
            },
            "required": ["source"]
        })),
        // The declared response render (the reminder.list pattern): the palette POSTS this envelope
        // (args interpolated into `source.args`) instead of discarding the bridge result — so the
        // snapshot lands in the channel as a durable, re-runnable, basket-attachable item. `jsonview`
        // renders it COLLAPSED to a one-line summary by default (a snapshot can be big; the reader
        // expands on demand). The UI stays tool-agnostic: it mounts whatever render is declared here.
        result: Some(json!({
            "v": 2,
            "view": "jsonview",
            "source": { "tool": "federation.sample", "args": {} },
            "options": { "collapsed": true },
            "tools": ["federation.sample"]
        })),
    }
}
