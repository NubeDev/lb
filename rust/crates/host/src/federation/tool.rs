//! The `federation.*` / `datasource.*` MCP bridge (datasources scope, README §6.5: the federation
//! control plane is reached as MCP tools under the one contract). The UI, rules, the AI agent, and
//! other extensions reach a federated source the SAME way they reach any tool — a qualified call.
//!
//! Each verb authorizes workspace-first inside its service function (the deny path); the bridge maps
//! the result/JSON. The sidecar-routed verbs (`federation.query`/`mirror`, `datasource.test`) use the
//! real `OsLauncher` — the same seam `install_native`/`call_sidecar` use; the store-only verbs
//! (`datasource.add`/`remove`/`list`) need no launcher.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_supervisor::OsLauncher;
use serde_json::{json, Value};

use super::{
    datasource_add, datasource_list, datasource_remove, datasource_test, federation_mirror,
    federation_query, federation_sample, federation_schema,
};
use crate::boot::Node;

/// Dispatch a `federation.*` / `datasource.*` MCP call. `input` is the verb's JSON args; the return
/// is the verb's JSON result. The MCP gate runs inside each service verb first (opaque `Denied`).
pub async fn call_federation_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    // A logical timestamp the caller may thread through (no wall-clock in core); default 0.
    let ts = input.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
    let launcher = OsLauncher;

    match qualified_tool {
        "federation.query" => {
            let source = str_arg(input, "source")?;
            let sql = str_arg(input, "sql")?;
            let out = federation_query(node, &launcher, principal, ws, source, sql, ts).await?;
            Ok(out)
        }
        "federation.schema" => {
            let source = str_arg(input, "source")?;
            // `table` is optional: absent → list tables, present → describe that table.
            let table = input.get("table").and_then(|v| v.as_str());
            let out = federation_schema(node, &launcher, principal, ws, source, table, ts).await?;
            Ok(out)
        }
        "federation.sample" => {
            let source = str_arg(input, "source")?;
            // `tables` filters the snapshot to the named tables; `limit` is rows/table (clamped).
            let tables: Option<Vec<String>> =
                input.get("tables").and_then(|v| v.as_array()).map(|a| {
                    a.iter()
                        .filter_map(|t| t.as_str().map(str::to_string))
                        .collect()
                });
            let limit = input.get("limit").and_then(|v| v.as_u64());
            let out = federation_sample(
                node,
                &launcher,
                principal,
                ws,
                source,
                tables.as_deref(),
                limit,
                ts,
            )
            .await?;
            Ok(out)
        }
        "federation.mirror" => {
            let source = str_arg(input, "source")?;
            let sql = str_arg(input, "query")?;
            let target = str_arg(input, "target_series")?;
            let job_id = str_arg(input, "job_id")?;
            // `range` is the max external rows to mirror (a bound); default a sane cap.
            let range = input
                .get("range")
                .and_then(|v| v.as_u64())
                .unwrap_or(10_000) as usize;
            let id = federation_mirror(
                node, &launcher, principal, ws, job_id, source, sql, target, range, ts,
            )
            .await?;
            Ok(json!({ "job_id": id }))
        }
        "datasource.add" => {
            let name = str_arg(input, "name")?;
            let kind = str_arg(input, "kind")?;
            let endpoint = str_arg(input, "endpoint")?;
            let secret_ref = input.get("secret_ref").and_then(|v| v.as_str());
            let dsn = input.get("dsn").and_then(|v| v.as_str());
            datasource_add(
                node, principal, ws, name, kind, endpoint, secret_ref, dsn, ts,
            )
            .await?;
            Ok(json!({ "ok": true }))
        }
        "datasource.remove" => {
            let name = str_arg(input, "name")?;
            datasource_remove(node, principal, ws, name, ts).await?;
            Ok(json!({ "ok": true }))
        }
        "datasource.list" => {
            let items = datasource_list(node, principal, ws).await?;
            Ok(json!({ "datasources": items }))
        }
        "datasource.test" => {
            let source = str_arg(input, "source")?;
            let out = datasource_test(node, &launcher, principal, ws, source, ts).await?;
            Ok(out)
        }
        _ => Err(ToolError::NotFound),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing string arg: {key}")))
}
