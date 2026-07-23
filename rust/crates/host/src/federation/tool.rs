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
    datasource_add, datasource_list, datasource_remove, datasource_test, dbschema_delete,
    dbschema_get, dbschema_list, dbschema_save, federation_delete, federation_export,
    federation_migrate, federation_mirror, federation_query, federation_sample, federation_schema,
    federation_write, ExportFrom,
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
            // The opt-in result-cache contract (`{ttl_s}`), passed through to the child verbatim.
            // Parsed EXPLICITLY here rather than riding along in a blob: the child input downstream
            // is built by enumeration, so an unparsed field validates against the schema and is then
            // silently dropped — a caching request that quietly does nothing.
            let cache = input.get("cache");
            // Trace id for correlating sub-queries of one dashboard panel refresh. Absent on
            // non-dashboard paths (query.run, mirror, query worker); empty string = no trace.
            let trace_id = input.get("trace_id").and_then(|v| v.as_str()).unwrap_or("");
            let out = federation_query(
                node,
                &launcher,
                principal,
                ws,
                source,
                sql,
                cache,
                ts,
                trace_id,
            )
            .await?;
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
        "federation.write" => {
            let source = str_arg(input, "source")?;
            let table = str_arg(input, "table")?;
            let columns: Vec<String> = input
                .get("columns")
                .and_then(|v| v.as_array())
                .ok_or_else(|| ToolError::BadInput("missing string-array arg: columns".into()))?
                .iter()
                .filter_map(|c| c.as_str().map(str::to_string))
                .collect();
            let rows: Vec<Value> = input
                .get("rows")
                .and_then(|v| v.as_array())
                .ok_or_else(|| ToolError::BadInput("missing array arg: rows".into()))?
                .clone();
            let key: Option<Vec<String>> = input.get("key").and_then(|v| v.as_array()).map(|a| {
                a.iter()
                    .filter_map(|c| c.as_str().map(str::to_string))
                    .collect()
            });
            let out = federation_write(
                node,
                &launcher,
                principal,
                ws,
                source,
                table,
                &columns,
                &rows,
                key.as_deref(),
                ts,
            )
            .await?;
            Ok(out)
        }
        "federation.delete" => {
            let source = str_arg(input, "source")?;
            let table = str_arg(input, "table")?;
            let key: Vec<String> = input
                .get("key")
                .and_then(|v| v.as_array())
                .ok_or_else(|| ToolError::BadInput("missing string-array arg: key".into()))?
                .iter()
                .filter_map(|c| c.as_str().map(str::to_string))
                .collect();
            let rows: Vec<Value> = input
                .get("rows")
                .and_then(|v| v.as_array())
                .ok_or_else(|| ToolError::BadInput("missing array arg: rows".into()))?
                .clone();
            let out = federation_delete(
                node, &launcher, principal, ws, source, table, &key, &rows, ts,
            )
            .await?;
            Ok(out)
        }
        "federation.migrate" => {
            let source = str_arg(input, "source")?;
            let schema = input
                .get("schema")
                .ok_or_else(|| ToolError::BadInput("missing object arg: schema".into()))?;
            let dry_run = input
                .get("dry_run")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let out =
                federation_migrate(node, &launcher, principal, ws, source, schema, dry_run, ts)
                    .await?;
            Ok(out)
        }
        "federation.export" => {
            let source = str_arg(input, "source")?;
            let job_id = str_arg(input, "job_id")?;
            let table = str_arg(input, "table")?;
            let from_series = input
                .get("from")
                .and_then(|v| v.get("series"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ToolError::BadInput("from.series is required (v1: series-only)".into())
                })?;
            let range = input
                .get("range")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize);
            let columns: Option<Vec<String>> =
                input.get("columns").and_then(|v| v.as_array()).map(|a| {
                    a.iter()
                        .filter_map(|c| c.as_str().map(str::to_string))
                        .collect()
                });
            let key: Option<Vec<String>> = input.get("key").and_then(|v| v.as_array()).map(|a| {
                a.iter()
                    .filter_map(|c| c.as_str().map(str::to_string))
                    .collect()
            });
            let id = federation_export(
                node,
                &launcher,
                principal,
                ws,
                job_id,
                source,
                &ExportFrom::Series {
                    name: from_series.to_string(),
                    range,
                },
                table,
                columns.as_deref(),
                key.as_deref(),
                ts,
            )
            .await?;
            Ok(json!({ "job_id": id }))
        }
        "dbschema.save" => {
            let name = str_arg(input, "name")?;
            let schema = input
                .get("schema")
                .ok_or_else(|| ToolError::BadInput("missing object arg: schema".into()))?;
            dbschema_save(node, principal, ws, name, schema, ts).await?;
            Ok(json!({ "ok": true }))
        }
        "dbschema.get" => {
            let name = str_arg(input, "name")?;
            let out = dbschema_get(node, principal, ws, name).await?;
            Ok(out.unwrap_or_else(|| json!({ "found": false })))
        }
        "dbschema.list" => {
            let out = dbschema_list(node, principal, ws).await?;
            Ok(out)
        }
        "dbschema.delete" => {
            let name = str_arg(input, "name")?;
            dbschema_delete(node, principal, ws, name, ts).await?;
            Ok(json!({ "ok": true }))
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
