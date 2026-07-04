//! `federation.query {source, sql}` → `{columns, rows}` — the read-first federation verb
//! (datasources scope). The host:
//!   1. authorizes `mcp:federation.query:call` (workspace-first — the deny path);
//!   2. resolves `{source}` to a `datasource:{ws}:{name}` record IN THE CALLER'S WORKSPACE
//!      (host-set, un-spoofable — a caller cannot name a cross-tenant source);
//!   3. re-validates SELECT-only host-side (defense in depth — the sidecar validates again);
//!   4. enforces `net:*` for the source's endpoint against the federation install grant (refused,
//!      opaque, if the grant omits it);
//!   5. mediates the DSN out of `lb-secrets` under the federation extension's OWN grant (never the
//!      caller's, never returned) and hands it + the SQL to the supervised sidecar.
//!
//! SurrealDB is never a DataFusion source (rule 2): this verb only reaches a registered EXTERNAL
//! source, never the platform store.

use lb_auth::Principal;
use lb_supervisor::Launcher;
use serde_json::{json, Value};

use super::error::FederationError;
use super::net::{enforce_endpoint, FEDERATION_EXT};
use super::secret::mediate_dsn;
use super::validate::validate_select_host;
use super::{authorize::authorize, record::resolve};
use crate::boot::Node;

/// Run `sql` against the registered `source` in `ws` as `caller`. Returns the sidecar's
/// `{columns, rows}` JSON value. `launcher` spawns/restarts the sidecar (the real path uses
/// `OsLauncher`); the call is routed through the native supervisor exactly like any sidecar tool.
pub async fn federation_query<L: Launcher>(
    node: &Node,
    launcher: &L,
    caller: &Principal,
    ws: &str,
    source: &str,
    sql: &str,
    ts: u64,
) -> Result<Value, FederationError> {
    authorize(caller, ws, "federation.query")?;

    // Resolve the alias to a registered source IN THIS workspace — un-spoofable (a cross-tenant name
    // resolves to nothing here, the wall made structural at the namespace).
    let ds = resolve(&node.store, ws, source)
        .await?
        .ok_or(FederationError::NotFound)?;

    // SELECT-only, host-side (the sidecar re-validates — two independent gates).
    validate_select_host(sql)?;

    // net:* — refuse, opaque, if the source's endpoint is not in the admin-approved grant.
    enforce_endpoint(&node.store, ws, &ds.endpoint).await?;

    // Mediate the DSN under the FEDERATION extension's own grant (never the caller's). The value is
    // handed child-ward in the call input — never returned, never logged.
    let dsn = mediate_dsn(node, ws, &ds.secret_ref).await?;

    let input = json!({
        "kind": ds.kind,
        "dsn": dsn,
        "source": source,
        "sql": sql,
    })
    .to_string();

    let out = crate::native::call_sidecar(
        node,
        launcher,
        caller,
        ws,
        FEDERATION_EXT,
        "federation.query",
        &input,
        ts,
    )
    .await
    .map_err(|e| FederationError::Sidecar(e.to_string()))?;

    serde_json::from_str(&out).map_err(|e| FederationError::Sidecar(e.to_string()))
}

/// The `tools.catalog` descriptor for `federation.query` — the palette's first tenant
/// (channels-command-palette scope). Declared in code next to the verb (FILE-LAYOUT); collected by
/// `tools::host_descriptors`. Carries the JSON-Schema input (`source`, `sql`) with the `x-lb-entity`
/// / `x-lb-widget` hints the palette renders its guided rail from.
///
/// **The OUTPUT half of the contract** (widget-platform scope, Slice C — closes G1): a `result = table`
/// envelope, the same shape `reminder/descriptor.rs::list_render()` established. The verb returns
/// `{columns, rows}` — the columnar shape `viz::frame::result_to_rows` is written for (column-aligned
/// arrays zipped into named row objects), so a `rich_result` table `source`-d at `federation.query`
/// renders unchanged through the shipped `WidgetView`, and Slice B's `dashboard.pin` mints a persisted
/// `pin-federation-query` cell from this envelope with ZERO federation-specific code in the pin path
/// (the mint treats `source.tool` as opaque data — rule 10). The `source.tool` names the tool itself
/// (the re-runnable read); the palette interpolates collected `source`/`sql` into `source.args`; the
/// bridge re-checks `mcp:federation.query:call` under the viewer's grant at render. `tools[]` is just
/// the read (a pure read has no row-control write verbs).
pub fn query_descriptor() -> lb_mcp::ToolDescriptor {
    lb_mcp::ToolDescriptor {
        name: "federation.query".to_string(),
        title: "Run a read-only SQL query against a registered datasource".to_string(),
        group: "federation".to_string(),
        input_schema: Some(crate::tools::federation_query_schema()),
        result: Some(query_result_render()),
    }
}

/// The interactive-table **response render envelope** `federation.query` declares (`x-lb-render`),
/// widget-platform Slice C. Same shape as `reminder::list_render`: the palette posts this verbatim
/// (interpolating collected args into `source.args`), the channel mounts it through the shipped
/// `WidgetView`, and `dashboard.pin` mints a persisted cell from it (generic over the tool id). The
/// source re-runs the verb at render under the viewer's grant; `viz::frame::result_to_rows` zips the
/// verb's `{columns, rows}` answer into named row objects. No `options.rowControls` (a pure read has
/// no row-control write verbs); no `fieldConfig` (the columns are caller-determined, so per-field
/// presentation is the viewer's choice, not the descriptor's).
pub(crate) fn query_result_render() -> serde_json::Value {
    serde_json::json!({
        "v": 2,
        "view": "table",
        "source": { "tool": "federation.query", "args": {} },
        "tools": ["federation.query"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The descriptor carries a `result = table` envelope (the OUTPUT contract) — same shape
    /// `reminder.list`'s render established. The source names the tool itself; the bridge re-runs it
    /// under the viewer's grant at render. Widget-platform Slice C headline.
    #[test]
    fn query_descriptor_carries_the_table_render() {
        let render = query_descriptor()
            .result
            .expect("federation.query declares a result render");
        assert_eq!(render["v"], 2);
        assert_eq!(render["view"], "table");
        assert_eq!(render["source"]["tool"], "federation.query");
        assert!(render["source"]["args"].is_object());
        // `tools[]` is just the read itself — a pure read has no row-control write verbs.
        let tools = render["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert!(tools.contains(&serde_json::json!("federation.query")));
    }
}
