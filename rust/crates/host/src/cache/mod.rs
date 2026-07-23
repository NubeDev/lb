//! The optional server-side response cache (response-cache scope) — a read-through, single-flight
//! cache wrapped around host-native MCP read-verb dispatch, AFTER auth + the caps wall, keyed
//! `{workspace, verb, canonical-args, generation}`. Two faces, cfg-selected on the `page-cache`
//! feature, so the ONE call site in `tool_call::dispatch_at_depth` never branches on the feature:
//!
//!   - **feature on:** the real moka cache (`live`), the verb-class policy, generations, the
//!     `cache.stats`/`cache.purge` verbs.
//!   - **feature off:** a zero-cost no-op — [`dispatch`] just runs the handler and serialises,
//!     byte-for-byte the pre-cache behaviour, and no `moka` enters the binary.
//!
//! **v1 allowlist:** `datasource.list`, `series.list`, `flows.list`, `flows.get`, `ext.list` —
//! the source-picker bundle, audited caller-independent. `viz.query` is DEFERRED: the grant-audit
//! proved it subject-filtered (per-target re-auth under the caller), so caching it under a
//! subject-free key would leak; it re-enters only once keyed safely (a `subject_scoped` class +
//! time-bucket quantisation), the named follow-up. See the session doc.

mod config;
pub use config::CacheConfig;

use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::Value;

use crate::boot::Node;

// ---- shared (always compiled): the compute path both faces run on a miss/passthrough ----

/// Run the real host-verb handler and serialise its `Value` to the JSON string the pipeline returns.
/// This is exactly what `dispatch_at_depth` used to do inline; extracting it lets the cache seam wrap
/// it lazily (compute only on a miss). Shared by both feature faces.
async fn compute_json(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    verb: &str,
    input: &Value,
    depth: u32,
) -> Result<String, ToolError> {
    // `run_host_verb` owns its args (so the cache seam can hand it a computed value lazily); clone the
    // borrowed `input` in — it is the small args object, and this runs only on a miss/passthrough.
    //
    // BOX the call: `run_host_verb` is the ~240-line host-verb fan-out, so its future is large. The
    // cache seam nests it under a couple of extra async layers (dispatch → compute_json → single-flight
    // init), and a chain of large stack-allocated futures overflows the debug-build worker stack.
    // Heap-allocating this one future keeps every caller's frame small — the same reason the viz
    // re-entry (`viz/query.rs`) boxes its recursive dispatch.
    let out = Box::pin(crate::tool_call::run_host_verb(
        node,
        principal,
        ws,
        verb,
        input.clone(),
        depth,
    ))
    .await?;
    serde_json::to_string(&out).map_err(|e| ToolError::Extension(e.to_string()))
}

// ---- feature ON ----

#[cfg(feature = "page-cache")]
mod fingerprint;
#[cfg(feature = "page-cache")]
mod generation;
#[cfg(feature = "page-cache")]
mod live;
#[cfg(feature = "page-cache")]
mod policy;
#[cfg(feature = "page-cache")]
mod quantise;
#[cfg(feature = "page-cache")]
mod verbs;

#[cfg(feature = "page-cache")]
pub use live::ResponseCache;
#[cfg(feature = "page-cache")]
pub use verbs::call_cache_tool;

/// The `Node`-held cache slot. Installed once at boot; read lock-free on the hot path.
#[cfg(feature = "page-cache")]
pub type CacheSlot = std::sync::OnceLock<Option<Arc<ResponseCache>>>;
/// Feature-off: a zero-sized slot. `Node` carries the field unconditionally (so its construction
/// sites don't cfg-branch), but it holds nothing.
#[cfg(not(feature = "page-cache"))]
pub type CacheSlot = ();

/// A fresh, empty slot — used at every `Node` construction site (feature-agnostic).
#[cfg(feature = "page-cache")]
pub fn new_slot() -> CacheSlot {
    std::sync::OnceLock::new()
}
#[cfg(not(feature = "page-cache"))]
pub fn new_slot() -> CacheSlot {}

/// The cache seam invoked from `dispatch_at_depth` for EVERY host-native verb, after the caps gate.
/// Feature-on: cache the allowlisted reads (single-flight), bump generations after writes, pass
/// everything else through. Only the OUTERMOST call (`depth == 0`) participates — a re-entrant
/// target dispatch (viz's per-target reads, a nested `flows.run`) always runs live.
#[cfg(feature = "page-cache")]
pub(crate) async fn dispatch(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    verb: &str,
    input: &Value,
    depth: u32,
) -> Result<String, ToolError> {
    let cache = match (depth, node.response_cache()) {
        (0, Some(c)) => c,
        // Nested call, or cache absent/disabled: behave exactly like the feature-off seam.
        _ => return compute_json(node, principal, ws, verb, input, depth).await,
    };

    if let Some(class) = policy::read_class(verb) {
        // The subject-scoped class (`viz.query`, slice 2) takes the fingerprinted + quantised path so a
        // warm frame can only ever be served to a caller whose grants would have computed it. The class
        // — not a verb name — decides this (rule 10: `is_subject_scoped` is a property of the policy
        // table, not an `if verb == "viz.query"` in the cache middleware).
        if policy::is_subject_scoped(class) {
            return dispatch_subject_scoped(&cache, node, principal, ws, verb, input, class, depth)
                .await;
        }
        cache
            .get_or_compute(
                ws,
                verb,
                input,
                class,
                compute_json(node, principal, ws, verb, input, depth),
            )
            .await
    } else {
        // A write (or an uncacheable read). Run it, then — only on success — bump the classes it
        // dirties so the invalidation lands the moment the write does.
        let out = compute_json(node, principal, ws, verb, input, depth).await;
        if out.is_ok() {
            for class in policy::dirties(verb) {
                cache.bump(ws, *class);
            }
        }
        out
    }
}

/// The subject-scoped cached path for `viz.query` (dashboard-query-acceleration scope, slice 2). Reads
/// the caller's freshness directive, quantises the range to the TTL bucket, folds the caller's
/// capability fingerprint into the key, and serves read-through + single-flight. Kept in its own
/// function (not inlined in [`dispatch`]) so the fingerprint + quantise + key discipline reads top to
/// bottom in one place — the reviewed seam the leak boundary rests on.
#[cfg(feature = "page-cache")]
async fn dispatch_subject_scoped(
    cache: &Arc<ResponseCache>,
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    verb: &str,
    input: &Value,
    class: policy::Class,
    depth: u32,
) -> Result<String, ToolError> {
    // Freshness is author-controlled and OFF by default (a live board bypasses): only a positive
    // top-level `cache.ttl_s` enables the gateway cache. Absent/`0` ⇒ passthrough — every open resolves
    // fresh (and slice-1 threads the same `ttl_s:0` to targets, so their result caches also bypass).
    let ttl_s = input
        .get("cache")
        .and_then(|c| c.get("ttl_s"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if ttl_s == 0 {
        return compute_json(node, principal, ws, verb, input, depth).await;
    }

    // Floor the range to the TTL bucket — the key AND the executed range share it (the cache never
    // serves a range it did not compute), so relative-window opens inside one bucket collapse to one.
    let quantised = quantise::quantise_viz_input(input, ttl_s);

    // The capability fingerprint over the panel this caller is about to resolve — the provable leak
    // boundary. `viz.query` accepts the panel under `panel` or as the input itself (a bare call).
    let panel = quantised.get("panel").unwrap_or(&quantised);
    let fingerprint = fingerprint::capability_fingerprint(principal, ws, panel);

    // Read-through + single-flight on the fingerprinted, quantised key. The compute runs on the SAME
    // quantised input, so a miss executes exactly the range the key names.
    cache
        .get_or_compute_scoped(
            ws,
            verb,
            &quantised,
            class,
            &fingerprint,
            compute_json(node, principal, ws, verb, &quantised, depth),
        )
        .await
}

// ---- feature OFF ----

/// Feature-off seam: no cache, no `moka` — just the handler + serialise. Identical behaviour to
/// today's binary; the optimiser inlines this to the bare call.
#[cfg(not(feature = "page-cache"))]
pub(crate) async fn dispatch(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    verb: &str,
    input: &Value,
    depth: u32,
) -> Result<String, ToolError> {
    compute_json(node, principal, ws, verb, input, depth).await
}

/// Feature-off: the `cache.*` verbs are not compiled in, so the tool genuinely does not exist here.
/// An already-authorized caller reaching this gets `NotFound` (the same as any unknown verb).
#[cfg(not(feature = "page-cache"))]
pub async fn call_cache_tool(
    _node: &Arc<Node>,
    _principal: &Principal,
    _ws: &str,
    _qualified_tool: &str,
    _input: &Value,
) -> Result<Value, ToolError> {
    Err(ToolError::NotFound)
}
