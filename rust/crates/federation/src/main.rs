//! `federation` — the native (Tier-2) datasources extension (datasources scope). A supervised OS
//! child that embeds DataFusion + its connectors as a LIBRARY and serves the control protocol
//! (`init`/`health`/`call`/`shutdown`) over `Content-Length`-framed stdio using the SAME
//! `lb-supervisor` wire types the host uses — so the child↔host ABI cannot drift.
//!
//! It is stateless (§3.4): it holds nothing durable. The DSN for a source is handed to it by the
//! HOST in each `call` input (host secret-mediation: the host pulls `secret:federation/{source}` and
//! passes it child-ward — never logged, never returned in a result). A kill + respawn loses nothing.
//!
//! Tools served (the rest of the federation surface — `datasource.add`/`remove`/`list`/`mirror` —
//! is HOST-side, this child only executes the engine-bound verbs):
//!   - `federation.query {kind, dsn, source, sql}` → `{columns, rows}` (SELECT-only, row-capped).
//!   - `datasource.test  {kind, dsn}`              → `{ok: true}` (a real connectivity probe).
//!
//! Attribution: the embedded-DataFusion + SQL-validator pattern is adapted from `rubix-cube`
//! (its `spice_engine` wrapper over the `datafusion` crate + its SQL validator), MIT/Apache-2.0.

mod info_schema;
mod migrate;
mod query;
mod sample;
mod source;
mod validate;
mod write;

use lb_supervisor::{read_frame, write_frame, CallParams, Method, Reply, Request};
use serde_json::{json, Value};
use tokio::io::{stdin, stdout};

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    let ext_id = std::env::var("LB_EXT_ID").unwrap_or_default();

    let mut input = stdin();
    let mut output = stdout();

    loop {
        let body = match read_frame(&mut input).await {
            Ok(b) => b,
            Err(_) => break, // host closed the line — exit cleanly
        };
        let req: Request = match serde_json::from_slice(&body) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let reply = match req.method {
            Method::Init => Reply::ok(req.id, format!(r#"{{"ready":true,"ext":"{ext_id}"}}"#)),
            Method::Health => Reply::ok(req.id, "ok"),
            Method::Shutdown => {
                let bytes = serde_json::to_vec(&Reply::ok(req.id, "bye")).unwrap();
                let _ = write_frame(&mut output, &bytes).await;
                break;
            }
            // Fenced in its own task so a PANIC deep in an engine/connector (live: a failed remote
            // execute took the whole child down, the supervisor burned its 5 restarts, and
            // federation went dark for the session) unwinds THAT task only — the caller gets an
            // error reply and the child keeps serving. An error `Reply` was always non-fatal; this
            // makes a panic equally non-fatal.
            Method::Call => {
                let id = req.id;
                match tokio::spawn(async move { handle_call(&req).await }).await {
                    Ok(reply) => reply,
                    Err(e) => Reply::err(id, format!("tool call panicked: {e}")),
                }
            }
        };

        let bytes = serde_json::to_vec(&reply).unwrap();
        if write_frame(&mut output, &bytes).await.is_err() {
            break;
        }
    }
}

/// Handle a `call`: parse the tool + input and dispatch to the engine. The input carries the
/// host-mediated DSN — used to open the pool and dropped, never echoed into the reply.
async fn handle_call(req: &Request) -> Reply {
    let params: CallParams = match serde_json::from_str(&req.params) {
        Ok(p) => p,
        Err(e) => return Reply::err(req.id, format!("bad params: {e}")),
    };
    let input: Value = match serde_json::from_str(&params.input) {
        Ok(v) => v,
        Err(e) => return Reply::err(req.id, format!("bad input json: {e}")),
    };

    match params.tool.as_str() {
        "federation.query" => federation_query(req.id, &input).await,
        "federation.schema" => federation_schema(req.id, &input).await,
        "federation.sample" => federation_sample(req.id, &input).await,
        "federation.write" => federation_write(req.id, &input).await,
        "federation.migrate" => federation_migrate(req.id, &input).await,
        "datasource.test" => datasource_test(req.id, &input).await,
        other => Reply::err(req.id, format!("unknown tool: {other}")),
    }
}

/// `federation.query` — run the validated SELECT against the source and return `{columns, rows}`.
async fn federation_query(id: u64, input: &Value) -> Reply {
    let (kind, dsn, sql) = match (
        str_of(input, "kind"),
        str_of(input, "dsn"),
        str_of(input, "sql"),
    ) {
        (Some(k), Some(d), Some(s)) => (k, d, s),
        _ => return Reply::err(id, "missing kind/dsn/sql"),
    };
    match query::run_query(kind, dsn, sql).await {
        Ok(r) => {
            let out = json!({ "columns": r.columns, "rows": r.rows });
            Reply::ok(id, out.to_string())
        }
        // The error string never includes the DSN (the source layer redacts it).
        Err(e) => Reply::err(id, e),
    }
}

/// `federation.schema` — native discovery (no `information_schema` SQL: the engine only registers
/// referenced tables). With no `table` arg it lists the source's user tables (each `{name, rows?}`);
/// with a `table` arg it returns that table's columns (`{columns:[{name,data_type,nullable}]}`),
/// read from the provider's real Arrow schema. The DSN is mediated by the host, same as query.
async fn federation_schema(id: u64, input: &Value) -> Reply {
    let (kind, dsn) = match (str_of(input, "kind"), str_of(input, "dsn")) {
        (Some(k), Some(d)) => (k, d),
        _ => return Reply::err(id, "missing kind/dsn"),
    };
    let table = str_of(input, "table");
    let result = match table {
        None => query::discover_tables(kind, dsn).await.map(|tables| {
            json!({ "tables": tables.iter().map(|t| {
                let mut o = json!({ "name": t.name });
                if let Some(rows) = t.rows { o["rows"] = json!(rows); }
                o
            }).collect::<Vec<_>>() })
        }),
        Some(table) => query::describe_table(kind, dsn, table).await.map(|cols| {
            json!({ "columns": cols.iter().map(|c| json!({
                "name": c.name, "data_type": c.data_type, "nullable": c.nullable
            })).collect::<Vec<_>>() })
        }),
    };
    match result {
        Ok(value) => Reply::ok(id, value.to_string()),
        Err(e) => Reply::err(id, e),
    }
}

/// `federation.sample` — one AI-ready snapshot (datasource-samples scope): every table's columns +
/// foreign keys + up to `limit` real rows, bounded and redacted in `sample::run_sample`. The DSN is
/// mediated by the host, same as query.
async fn federation_sample(id: u64, input: &Value) -> Reply {
    let (kind, dsn) = match (str_of(input, "kind"), str_of(input, "dsn")) {
        (Some(k), Some(d)) => (k, d),
        _ => return Reply::err(id, "missing kind/dsn"),
    };
    let tables: Option<Vec<String>> = input.get("tables").and_then(|v| v.as_array()).map(|a| {
        a.iter()
            .filter_map(|t| t.as_str().map(str::to_string))
            .collect()
    });
    let limit = input
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(sample::DEFAULT_ROWS);
    match sample::run_sample(kind, dsn, tables, limit).await {
        Ok(value) => Reply::ok(id, value.to_string()),
        Err(e) => Reply::err(id, e),
    }
}

/// `datasource.test` — a real connectivity probe; `{ok:true}` is green.
async fn datasource_test(id: u64, input: &Value) -> Reply {
    let (kind, dsn) = match (str_of(input, "kind"), str_of(input, "dsn")) {
        (Some(k), Some(d)) => (k, d),
        _ => return Reply::err(id, "missing kind/dsn"),
    };
    match query::probe(kind, dsn).await {
        Ok(()) => Reply::ok(id, json!({ "ok": true }).to_string()),
        Err(e) => Reply::err(id, e),
    }
}

/// `federation.write {kind, dsn, table, columns, rows, key?}` — bounded INSERT/UPSERT (schema-
/// designer scope). The host resolves the source + mediates the DSN; this sidecar generates the
/// parameterized SQL and runs it through `Source::write_rows`. Row-capped; past the cap the error
/// steers to `federation.export`. The DSN never appears in the reply.
async fn federation_write(id: u64, input: &Value) -> Reply {
    let (kind, dsn, table) = match (
        str_of(input, "kind"),
        str_of(input, "dsn"),
        str_of(input, "table"),
    ) {
        (Some(k), Some(d), Some(t)) => (k, d, t),
        _ => return Reply::err(id, "missing kind/dsn/table"),
    };
    let columns: Vec<String> = match input.get("columns").and_then(|v| v.as_array()) {
        Some(a) => a
            .iter()
            .filter_map(|c| c.as_str().map(str::to_string))
            .collect(),
        None => return Reply::err(id, "missing columns"),
    };
    let rows: Vec<Value> = input
        .get("rows")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let key: Option<Vec<String>> = input.get("key").and_then(|v| v.as_array()).map(|a| {
        a.iter()
            .filter_map(|c| c.as_str().map(str::to_string))
            .collect()
    });

    let key_ref: Option<&[String]> = key.as_deref();
    match write::run_write(kind, dsn, table, &columns, &rows, key_ref).await {
        Ok(affected) => {
            let out = json!({ "affected": affected });
            Reply::ok(id, out.to_string())
        }
        Err(e) => Reply::err(id, e),
    }
}

/// `federation.migrate {kind, dsn, schema, dry_run?}` — diff the desired schema vs the live
/// catalog, plan additive DDL, and (when `dry_run` is false) apply it (schema-designer scope). The
/// host resolves the source + mediates the DSN; this sidecar reads the live catalog, plans via the
/// pure `dialect::plan_migrate`, and applies via `Source::apply_ddl` in one transaction.
async fn federation_migrate(id: u64, input: &Value) -> Reply {
    let (kind, dsn) = match (str_of(input, "kind"), str_of(input, "dsn")) {
        (Some(k), Some(d)) => (k, d),
        _ => return Reply::err(id, "missing kind/dsn"),
    };
    let Some(schema_value) = input.get("schema") else {
        return Reply::err(id, "missing schema");
    };
    let schema: source::DesignSchema = match serde_json::from_value(schema_value.clone()) {
        Ok(s) => s,
        Err(e) => return Reply::err(id, format!("bad schema: {e}")),
    };
    let dry_run = input
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    match migrate::run_migrate(kind, dsn, &schema, dry_run).await {
        Ok(value) => Reply::ok(id, value.to_string()),
        Err(e) => Reply::err(id, e),
    }
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(|x| x.as_str())
}
