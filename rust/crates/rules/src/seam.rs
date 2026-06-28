//! The three re-seamed boundaries (rules-engine-scope "Source: ported, not copied"). rubix-cube
//! collected grids through a local DataFusion engine and called an LLM over `reqwest`; lb-rules has
//! NEITHER in this crate. Instead the host implements these traits against the real chokepoints:
//!   - [`DataSeam`] — collect a composed query. A PLATFORM source resolves to `store.query`/`series.*`
//!     (SurrealDB, authoritative); an EXTERNAL source resolves to `federation.query` (the datasources
//!     extension). The rule author sees one `source(...)` surface; the seam picks the path.
//!   - [`AiSeam`] — the AI-gateway (`ModelAccess`), keeping rubix-cube's budget meter + nsql fence.
//!
//! The verbs are SYNCHRONOUS rhai closures running on a blocking thread; the seam methods are sync too.
//! The host's impl bridges to its async store/gateway via a `tokio::runtime::Handle::block_on` inside
//! the impl — exactly rubix-cube's grid pattern (the engine call was `handle.block_on(...)`).

use std::collections::BTreeMap;

use crate::runtime::GridJson;

/// Which authoritative path a source resolves to. Decided host-side from the registered source kind;
/// the rule author never picks — `source("series")` and `source("timescale")` read alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// Platform data — collect via `store.query` / `series.*` (SurrealDB, native). SurrealQL dialect.
    Platform,
    /// External data — collect via `federation.query` (the datasources extension). ANSI SQL dialect.
    Federation,
}

/// A column's name + a coarse type, for `ai.ask` schema introspection (the model proposes SQL against
/// the workspace's OWN sources only — never a cross-tenant table).
#[derive(Debug, Clone)]
pub struct SchemaColumn {
    pub name: String,
    pub ty: String,
}

/// The data-collect seam. Implemented by the host; closed over per run with the workspace pinned.
pub trait DataSeam: Send + Sync {
    /// Resolve `source` to its kind WITHIN this run's workspace, enforcing the allowlist. Returns the
    /// kind + the resolved physical name. `Err` if the source is not granted (opaque deny upstream).
    fn resolve(&self, source: &str) -> Result<(SourceKind, String), String>;

    /// Collect a composed query against `source` of `kind`, returning columns + rows. PLATFORM →
    /// `store.query`/`series.*`; FEDERATION → `federation.query`. The host re-runs its `caps::check`
    /// + workspace pin here (the chokepoint that replaces rubix-cube's per-collect SQL validator).
    fn collect(&self, kind: SourceKind, source: &str, query: &str) -> Result<GridJson, String>;

    /// The column schemas of the workspace's granted sources — for `ai.ask` nsql prompting. Never
    /// lists a source outside the allowlist (mandatory isolation).
    fn schemas(&self) -> Result<BTreeMap<String, Vec<SchemaColumn>>, String>;
}

/// The model-access seam — re-points rubix-cube's `AiBackend` at the AI-gateway (`ModelAccess`). The
/// rule never sees a key; spend is metered where the platform already meters it.
pub trait AiSeam: Send + Sync {
    /// A free-form completion. `tokens` is reported back for the budget meter.
    fn complete(&self, prompt: &str) -> Result<AiCompletion, String>;

    /// Propose SQL for a natural-language question, given the workspace's schemas. The RESULT is
    /// re-validated through [`DataSeam::collect`]'s gate before it ever runs (the nsql fence) — there
    /// is no path from a proposed query to execution that skips the validator.
    fn propose_sql(
        &self,
        question: &str,
        schemas: &BTreeMap<String, Vec<SchemaColumn>>,
    ) -> Result<String, String>;

    /// An embedding vector. Optional; default errors "not supported".
    fn embed(&self, _text: &str) -> Result<Vec<f64>, String> {
        Err("ai.embed not supported by this backend".into())
    }
}

/// A completion + its token count.
#[derive(Debug, Clone)]
pub struct AiCompletion {
    pub text: String,
    pub tokens: u32,
}
