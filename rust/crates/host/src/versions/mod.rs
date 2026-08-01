//! The **entity version history** service — a capped, per-entity ring of full after-image snapshots
//! of every dashboard, flow, and rule, with list / get / restore verbs
//! (`docs/scope/versions/entity-version-history-scope.md`, `NubeDev/lb#112`).
//!
//! Not undo. The undo journal is a per-actor, linear, conditional-restore stack; this is per-entity,
//! addressable, restore-anything history. They compose (a restore is itself undoable) and share no
//! storage — only the depth-0 dispatch seam that feeds both.
//!
//! Files (FILE-LAYOUT §3 — one responsibility each):
//!   - `plan`       — the KIND PLAN TABLE (`kind → table, save_tool, id keys, counter field`) and
//!     the pure classification of a dispatched call. Adding a kind is adding a row.
//!   - `record`     — the `entity_version` row, the ring key, the stable snapshot hash.
//!   - `cap`        — the adjustable cap: const → workspace → per-kind, node-clamped.
//!   - `store`      — the only place the ring table is queried.
//!   - `capture`    — the depth-0 after-image capture (sibling of `undo_capture`).
//!   - `list` / `get` / `restore` / `config` — one verb per file.
//!   - `descriptor` — the JSON Schemas the catalog and the arg validator serve.
//!   - `error`      — the service error, with the opaque denial.

use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::{json, Value};

use crate::Node;

mod cap;
mod capture;
mod config;
mod descriptor;
mod error;
mod get;
mod list;
mod plan;
mod record;
mod restore;
mod store;

pub(crate) use capture::capture_version;
pub use descriptor::descriptors;
pub(crate) use plan::table_for_kind;

pub use cap::DEFAULT_VERSION_CAP;
pub use error::VersionsError;
pub use record::TABLE as ENTITY_VERSION_TABLE;

/// The restore verb's qualified name. Named once because THREE places must agree on it: the
/// dispatch arm, the capture classifier (a restore is captured as the entity write it performs),
/// and `undo_capture`'s plan.
pub const RESTORE_TOOL: &str = "versions.restore";

/// Dispatch a `versions.<verb>` MCP call. The outer gate in `tool_call.rs` already ran
/// `mcp:versions.<verb>:call`; each verb re-runs its own gate inside (defense in depth, and the same
/// function the gateway routes call), plus restore's no-escalation check on the kind's save cap.
///
/// `depth` is threaded through because `versions.restore` RE-ENTERS the dispatcher to run the kind's
/// own save verb — the same pattern the `viz.` resolver and `series.producer.health` use.
pub async fn call_versions_tool(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
    depth: u32,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "versions.list" => {
            let out = list::versions_list(
                &node.store,
                principal,
                ws,
                str_arg(input, "kind")?,
                str_arg(input, "id")?,
                input
                    .get("limit")
                    .and_then(Value::as_u64)
                    .map(|n| n as usize),
            )
            .await?;
            Ok(json!({ "versions": out.versions, "cap": out.cap }))
        }
        "versions.get" => {
            let v = get::versions_get(
                &node.store,
                principal,
                ws,
                str_arg(input, "kind")?,
                str_arg(input, "id")?,
                str_arg(input, "version_id")?,
            )
            .await?;
            // `version` is the metadata projection and `snapshot` the content — the same split the
            // list verb uses, so a client has ONE row shape everywhere. `is_head` is false here by
            // construction: the marker is a list-level fact (see `list.rs`), and computing it on a
            // single get would cost a second read for a flag the caller already has.
            Ok(json!({ "version": v.meta(false), "snapshot": v.snapshot }))
        }
        "versions.restore" => {
            let out = restore::versions_restore(
                node,
                principal,
                ws,
                str_arg(input, "kind")?,
                str_arg(input, "id")?,
                str_arg(input, "version_id")?,
                input.get("now").and_then(Value::as_u64),
                depth,
            )
            .await?;
            Ok(serde_json::to_value(out).unwrap_or(Value::Null))
        }
        "versions.config.get" => {
            let v = config::versions_config_get(&node.store, principal, ws).await?;
            Ok(serde_json::to_value(v).unwrap_or(Value::Null))
        }
        "versions.config.set" => {
            let v = config::versions_config_set(&node.store, principal, ws, input).await?;
            Ok(serde_json::to_value(v).unwrap_or(Value::Null))
        }
        _ => Err(ToolError::NotFound),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::BadInput(format!("missing/!string arg: {key}")))
}
