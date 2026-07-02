//! The `control-engine.*` verb dispatch (folder-of-verbs, one file per verb).
//!
//! `dispatch` maps a manifest tool NAME (the cap gate — house rule) + its parsed
//! input to one `ControlEngine` trait call and returns the verbatim serde JSON
//! result. It is the seam the crate's own unit tests drive against `ce_fake` (with
//! its call counter) to prove dispatch + verbatim-DTO behaviour without a process.
//!
//! Deny is enforced HOST-side on the tool name (via `authorize_tool`) BEFORE the
//! sidecar is ever called, so a denied call reaches neither `dispatch` nor the CE —
//! the crate unit test asserts that dispatch invokes the trait exactly once per
//! allowed call (counter semantics) while the host integration test asserts the
//! `Denied` at the `call_tool` boundary.

pub mod appliance;
pub mod schema;
pub mod tree;

use rubix_ce::{ControlEngine, EngineInstanceId};
use serde_json::Value;

/// Dispatch one `control-engine.*` verb against a bound CE client.
///
/// `tool` is the full manifest tool name; `input` is the already-parsed argument
/// object. Returns the verb's verbatim JSON result, or an error string (mapped by
/// `main` onto a `Reply::err`). Unknown tools error — the host only ever routes the
/// declared names, so this is a defensive fallback.
pub async fn dispatch(
    engine: &dyn ControlEngine,
    instance: &EngineInstanceId,
    tool: &str,
    input: &Value,
) -> Result<Value, String> {
    match tool {
        "control-engine.tree" => tree::run(engine, instance, input).await,
        "control-engine.schema" => schema::run(engine).await,
        other => Err(format!("unknown tool: {other}")),
    }
}

// ---------------------------------------------------------------------------------
// Crate-level unit tests: the graph dispatch layer's deny-before-call semantics + verbatim DTO shape,
// driven against the ONE sanctioned fake (`ce_fake`, with its call counter). These prove "0 trait
// calls before a denied call" at the dispatch seam (the host integration test proves the `Denied` at
// the call_tool boundary). No process, no store, no bus — just dispatch × the trait.
// ---------------------------------------------------------------------------------
#[cfg(test)]
mod dispatch_tests {
    use super::dispatch;
    use crate::ce_fake::CeFake;
    use rubix_ce::EngineInstanceId;
    use serde_json::json;
    use std::sync::atomic::Ordering;

    #[tokio::test]
    async fn tree_returns_seeded_graph_verbatim_and_counts_one_call() {
        let fake = CeFake::seeded();
        let inst = EngineInstanceId::edge();

        assert_eq!(
            fake.calls.load(Ordering::SeqCst),
            0,
            "no trait call before dispatch"
        );

        let out = dispatch(
            &*fake,
            &inst,
            "control-engine.tree",
            &json!({ "appliance": "" }),
        )
        .await
        .expect("tree dispatches");

        let nodes = out["nodes"].as_array().expect("nodes array");
        assert_eq!(nodes.len(), 1, "one seeded node: {out}");
        assert_eq!(nodes[0]["uid"], 1);
        assert_eq!(nodes[0]["type"], "test-math::add");
        assert_eq!(out["edges"].as_array().expect("edges array").len(), 0);

        assert_eq!(
            fake.calls.load(Ordering::SeqCst),
            1,
            "one trait call per dispatch"
        );
    }

    #[tokio::test]
    async fn schema_returns_manifest_list_verbatim() {
        let fake = CeFake::seeded();
        let inst = EngineInstanceId::edge();
        let out = dispatch(
            &*fake,
            &inst,
            "control-engine.schema",
            &json!({ "appliance": "" }),
        )
        .await
        .expect("schema dispatches");
        let mans = out["manifests"].as_array().expect("manifests array");
        assert_eq!(mans.len(), 1, "one seeded manifest: {out}");
        assert_eq!(mans[0]["vendor"], "test");
        assert_eq!(mans[0]["name"], "math");
        assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn unknown_tool_errors_without_a_trait_call() {
        let fake = CeFake::seeded();
        let inst = EngineInstanceId::edge();
        let err = dispatch(&*fake, &inst, "control-engine.bogus", &json!({}))
            .await
            .expect_err("unknown tool errors");
        assert!(err.contains("unknown tool"), "got: {err}");
        assert_eq!(
            fake.calls.load(Ordering::SeqCst),
            0,
            "an unknown tool makes NO trait call (deny-before-call at dispatch)"
        );
    }
}
