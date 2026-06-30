//! The re-seam — `HostDataSeam` + `HostAiSeam` (rules-engine-scope "Source: ported, not copied"). The
//! lb-rules engine runs on a blocking thread and calls these SYNCHRONOUS seam methods; each bridges to
//! the host's async surface via a captured `tokio::runtime::Handle::block_on`:
//!   - PLATFORM collect → `store.query` (`store_query_run`, SurrealDB, authoritative);
//!   - FEDERATION collect → the `federation.query` MCP verb on the registry (the datasources ext);
//!   - AI → the AI-gateway (`ModelAccess`), keeping the budget + the nsql fence (the fence is the
//!     re-validation through `collect`, enforced inside the lb-rules `ai` verb).
//!
//! Every collect re-runs the host gate (workspace pin + `caps::check`) — the chokepoint that replaces
//! rubix-cube's per-collect SQL validator. The seam is closed over the caller's effective principal +
//! the pinned workspace, so a rule can read no source a direct query in the same workspace couldn't.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use lb_auth::Principal;
use lb_rules::seam::{AiSeam, DataSeam, SchemaColumn, SourceKind};
use lb_rules::{AiCompletion, GridJson};
use serde_json::{json, Value};
use tokio::runtime::Handle;

use crate::boot::Node;

/// The platform pseudo-source a rule reads the series plane through.
pub const PLATFORM_SERIES: &str = "series";
/// The platform pseudo-source a rule reads arbitrary store tables through (via `store.query`).
pub const PLATFORM_STORE: &str = "store";

/// The host data seam for one run. Resolves a source name to platform vs federation, collects through
/// the right host path, and lists the workspace's own schemas for `ai.ask`.
pub struct HostDataSeam {
    node: Arc<Node>,
    principal: Principal,
    ws: String,
    handle: Handle,
    /// The federation datasource names registered in this workspace (resolved once at construction).
    datasources: HashSet<String>,
    /// The saved-query ids registered in this workspace — the `source("query:<id>")` allowlist (query
    /// scope: a rule reuses a saved query by name, re-checked under caller ∩ grant at collect).
    queries: HashSet<String>,
}

impl HostDataSeam {
    pub fn new(
        node: Arc<Node>,
        principal: Principal,
        ws: String,
        handle: Handle,
        datasources: HashSet<String>,
        queries: HashSet<String>,
    ) -> Self {
        Self {
            node,
            principal,
            ws,
            handle,
            datasources,
            queries,
        }
    }

    /// The set of source names this run may read — the platform pseudo-sources + the workspace's
    /// registered federation datasources + its saved queries (`query:<id>`).
    pub fn allowed_sources(&self) -> HashSet<String> {
        let mut s = self.datasources.clone();
        s.insert(PLATFORM_SERIES.to_string());
        s.insert(PLATFORM_STORE.to_string());
        for id in &self.queries {
            s.insert(format!("query:{id}"));
        }
        s
    }

    fn collect_platform(&self, query: &str) -> Result<GridJson, String> {
        let node = self.node.clone();
        let principal = self.principal.clone();
        let ws = self.ws.clone();
        let q = query.to_string();
        let result = self.handle.block_on(async move {
            crate::store_query_run(&node.store, &principal, &ws, &q, Vec::new()).await
        });
        match result {
            Ok(r) => Ok(GridJson {
                columns: r.columns,
                rows: r.rows,
            }),
            // Keep the deny opaque via our "source not allowed" classification upstream.
            Err(crate::StoreQueryError::Denied) => Err("source not allowed".into()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Collect a SAVED QUERY by id (`source("query:<id>")`, query scope). Routes through the ONE MCP
    /// contract — `query.run` — so the caller's `mcp:query.run:call` AND the target's underlying cap
    /// are re-checked inside the call (caller ∩ grant, the existing per-source chokepoint). A saved
    /// query is thus a centrally-editable data definition a rule composes by name, not re-implements.
    fn collect_query(&self, id: &str) -> Result<GridJson, String> {
        let node = self.node.clone();
        let principal = self.principal.clone();
        let ws = self.ws.clone();
        let input = json!({ "id": id }).to_string();
        let result = self.handle.block_on(async move {
            crate::call_tool(&node, &principal, &ws, "query.run", &input).await
        });
        let out = match result {
            Ok(o) => o,
            Err(lb_mcp::ToolError::Denied) => return Err("source not allowed".to_string()),
            Err(e) => return Err(format!("query.run failed: {e}")),
        };
        let val: Value = serde_json::from_str(&out).map_err(|e| e.to_string())?;
        let columns = val
            .get("columns")
            .and_then(|c| c.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let rows = val
            .get("rows")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(GridJson { columns, rows })
    }

    /// Resolve a saved query's `SourceKind` from ITS target (host-side, in the caller's workspace). The
    /// rule author never picks — `source("query:<id>")` reads alike whether the query targets the
    /// platform store or a datasource. A missing/removed query is "not allowed".
    fn query_kind(&self, id: &str) -> Result<SourceKind, String> {
        let node = self.node.clone();
        let ws = self.ws.clone();
        let id_owned = id.to_string();
        let q = self
            .handle
            .block_on(async move { crate::query::resolve_query(&node.store, &ws, &id_owned).await })
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("source not allowed: query:{id}"))?;
        match crate::query::QueryTarget::parse(&q.target) {
            Ok(crate::query::QueryTarget::Datasource(_)) => Ok(SourceKind::Federation),
            _ => Ok(SourceKind::Platform),
        }
    }

    fn collect_federation(&self, source: &str, query: &str) -> Result<GridJson, String> {
        let node = self.node.clone();
        let principal = self.principal.clone();
        let ws = self.ws.clone();
        let input = json!({ "source": source, "sql": query }).to_string();
        // Route through the one MCP contract: `federation.query` on the registry (the datasources ext).
        let result = self.handle.block_on(async move {
            crate::call_tool(&node, &principal, &ws, "federation.query", &input).await
        });
        // A capability/workspace deny stays opaque ("source not allowed" → SourceNotAllowed → the MCP
        // Denied). Any OTHER fault — a sidecar SQL/planning error, a bad input — is AUTHOR FEEDBACK and
        // must surface verbatim (the workbench "BadInput verbatim, Denied opaque" honesty rule), never
        // masked as a permission deny. The message deliberately avoids the "source not allowed" substring
        // so the engine's `map_eval_error` classifies it as `Eval` (shown) rather than a deny.
        let out = match result {
            Ok(o) => o,
            Err(lb_mcp::ToolError::Denied) => return Err("source not allowed".to_string()),
            Err(e) => return Err(format!("federation query failed: {e}")),
        };
        let val: Value = serde_json::from_str(&out).map_err(|e| e.to_string())?;
        let columns = val
            .get("columns")
            .and_then(|c| c.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let rows = val
            .get("rows")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(GridJson { columns, rows })
    }
}

impl DataSeam for HostDataSeam {
    fn resolve(&self, source: &str) -> Result<(SourceKind, String), String> {
        if let Some(id) = source.strip_prefix("query:") {
            // A saved query — kind is decided by ITS target (resolved in the caller's workspace), not
            // picked by the rule author. collect re-runs query.run under caller ∩ grant.
            if !self.queries.contains(id) {
                return Err(format!("source not allowed: {source}"));
            }
            let kind = self.query_kind(id)?;
            return Ok((kind, source.to_string()));
        }
        if self.datasources.contains(source) {
            Ok((SourceKind::Federation, source.to_string()))
        } else if source == PLATFORM_SERIES {
            // The series plane table — read through store.query.
            Ok((SourceKind::Platform, lb_ingest::SERIES_TABLE.to_string()))
        } else if source == PLATFORM_STORE {
            Ok((SourceKind::Platform, PLATFORM_STORE.to_string()))
        } else {
            Err(format!("source not allowed: {source}"))
        }
    }

    fn collect(&self, kind: SourceKind, source: &str, query: &str) -> Result<GridJson, String> {
        // A saved-query source ignores the rule's composed SQL — it runs the saved query verbatim
        // (the centrally-edited definition), re-checked under caller ∩ grant inside query.run.
        if let Some(id) = source.strip_prefix("query:") {
            let _ = (kind, query);
            return self.collect_query(id);
        }
        match kind {
            SourceKind::Platform => self.collect_platform(query),
            SourceKind::Federation => self.collect_federation(source, query),
        }
    }

    fn schemas(&self) -> Result<BTreeMap<String, Vec<SchemaColumn>>, String> {
        // name (a coarse schema); platform schema discovery via store.schema is additive later. Never
        // lists a source outside this workspace (the set is workspace-resolved).
        let mut out = BTreeMap::new();
        for ds in &self.datasources {
            out.insert(ds.clone(), Vec::new());
        }
        out.insert(
            PLATFORM_SERIES.to_string(),
            vec![
                SchemaColumn {
                    name: "ts".into(),
                    ty: "datetime".into(),
                },
                SchemaColumn {
                    name: "value".into(),
                    ty: "number".into(),
                },
                SchemaColumn {
                    name: "series".into(),
                    ty: "string".into(),
                },
            ],
        );
        Ok(out)
    }
}

/// The host AI seam — re-points at the AI-gateway. Held behind a trait object the host builds from its
/// `ModelAccess` impl; for v1 the rule's `ai.complete`/`ai.ask` route to a single-turn gateway call.
pub struct HostAiSeam {
    inner: Arc<dyn RuleModel>,
}

impl HostAiSeam {
    pub fn new(inner: Arc<dyn RuleModel>) -> Self {
        Self { inner }
    }
}

/// A minimal model surface the rule engine needs — a single completion + an nsql proposal. Implemented
/// by the host over the AI-gateway `ModelAccess` (or the deterministic mock provider in tests). This is
/// the sanctioned external-behind-a-trait boundary (testing §0): the model is the one true external.
pub trait RuleModel: Send + Sync {
    fn complete(&self, prompt: &str) -> Result<(String, u32), String>;
    fn propose_sql(&self, question: &str, schema_hint: &str) -> Result<String, String>;
}

impl AiSeam for HostAiSeam {
    fn complete(&self, prompt: &str) -> Result<AiCompletion, String> {
        let (text, tokens) = self.inner.complete(prompt)?;
        Ok(AiCompletion { text, tokens })
    }

    fn propose_sql(
        &self,
        question: &str,
        schemas: &BTreeMap<String, Vec<SchemaColumn>>,
    ) -> Result<String, String> {
        let hint = schemas
            .iter()
            .map(|(name, cols)| {
                let c = cols
                    .iter()
                    .map(|c| format!("{}:{}", c.name, c.ty))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name}({c})")
            })
            .collect::<Vec<_>>()
            .join("; ");
        self.inner.propose_sql(question, &hint)
    }
}

/// Read the workspace's registered federation datasource names (for the allowlist + schema). Empty if
/// the datasources extension/records aren't present — platform-only rules still run.
/// The federation datasource names registered in this workspace — the allowlist a rule's
/// `source(...)`/`query(...)` resolves against. Reads the unwrapped record `data` values via
/// `lb_store::list` (NOT raw `lb_store::scan`, whose `Row.data` is the Versioned `{rev, data:{…}}`
/// envelope — reading `row.data.name` there always misses, emptying the allowlist and making every
/// federation source resolve as `SourceNotAllowed` → opaque `Denied`). Mirrors `federation/list.rs`.
pub async fn workspace_datasources(node: &Node, ws: &str) -> HashSet<String> {
    let rows = match lb_store::list(
        &node.store,
        ws,
        crate::federation::TABLE,
        "tag",
        &crate::federation::datasource_tag(),
    )
    .await
    {
        Ok(p) => p,
        Err(_) => return HashSet::new(),
    };
    let mut out = HashSet::new();
    for value in rows {
        if let Some(ds) = serde_json::from_value::<crate::federation::Datasource>(value).ok() {
            if !ds.removed {
                out.insert(ds.name);
            }
        }
    }
    out
}

/// Read the workspace's saved-query ids (for the `source("query:<id>")` allowlist). Empty if none —
/// the rule's other sources still resolve. Mirrors `workspace_datasources` over the query record's tag.
pub async fn workspace_queries(node: &Node, ws: &str) -> HashSet<String> {
    let rows = match lb_store::list(
        &node.store,
        ws,
        crate::query::TABLE,
        "tag",
        &crate::query::query_tag(),
    )
    .await
    {
        Ok(p) => p,
        Err(_) => return HashSet::new(),
    };
    let mut out = HashSet::new();
    for value in rows {
        if let Some(q) = serde_json::from_value::<crate::query::SavedQuery>(value).ok() {
            if !q.removed {
                out.insert(q.id);
            }
        }
    }
    out
}
