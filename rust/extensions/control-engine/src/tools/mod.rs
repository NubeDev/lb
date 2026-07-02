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

pub mod add_edge;
pub mod add_node;
pub mod appliance;
pub mod call_action;
pub mod clear_override;
pub mod patch;
pub mod remove_node;
pub mod schema;
pub mod set_override;
pub mod tree;

use rubix_ce::{ControlEngine, EngineInstanceId};
use serde_json::Value;

use crate::host::{HostCtx, HostError};

/// True iff `tool` is one of the seven S5 graph WRITE verbs. These take a different
/// dispatch path than reads: they thread a [`HostCtx`] so each verb can self-check
/// its own per-verb cap FIRST (the inbound `native.call` carries no caller identity).
#[must_use]
pub fn is_write_verb(tool: &str) -> bool {
    matches!(
        tool,
        "control-engine.add-node"
            | "control-engine.patch"
            | "control-engine.set-override"
            | "control-engine.clear-override"
            | "control-engine.add-edge"
            | "control-engine.remove-node"
            | "control-engine.call-action"
    )
}

/// Dispatch one `control-engine.*` graph WRITE verb against a bound CE client.
///
/// Unlike [`dispatch`] (reads), each write verb self-checks its own
/// `mcp:control-engine.<verb>:call` grant on `host` BEFORE resolving args or calling
/// the trait — defense-in-depth for the direct inbound path (the host's
/// `authorize_tool` on the routed/native.call boundary is the other half; both hold).
pub async fn dispatch_write(
    host: &HostCtx,
    engine: &dyn ControlEngine,
    instance: &EngineInstanceId,
    tool: &str,
    input: &Value,
) -> Result<Value, HostError> {
    match tool {
        "control-engine.add-node" => add_node::run(host, engine, instance, input).await,
        "control-engine.patch" => patch::run(host, engine, instance, input).await,
        "control-engine.set-override" => set_override::run(host, engine, instance, input).await,
        "control-engine.clear-override" => clear_override::run(host, engine, instance, input).await,
        "control-engine.add-edge" => add_edge::run(host, engine, instance, input).await,
        "control-engine.remove-node" => remove_node::run(host, engine, instance, input).await,
        "control-engine.call-action" => call_action::run(host, engine, instance, input).await,
        other => Err(HostError::BadInput(format!("unknown write verb: {other}"))),
    }
}

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

    // ── S5 write-verb dispatch: each maps to ONE trait call (counter 0→1), self-checks its own cap
    //    FIRST (missing cap → Denied, NO trait call), and validates the required node arg. Driven
    //    against `CeFake` + a hand-built `HostCtx` (no process, no store, no bus). ─────────────────
    use crate::host::{HostCtx, HostError};
    use lb_sidecar_client::{Config, SidecarClient};

    /// A `HostCtx` granting exactly `verbs` (each as `mcp:<verb>:call`), over a
    /// `SidecarClient` that is never called (the write path only reaches the callback
    /// for reads — write verbs go straight to the CE trait after the self-check).
    fn host_granting(verbs: &[&str]) -> HostCtx {
        let caps = verbs.iter().map(|v| format!("mcp:{v}:call")).collect();
        // A dummy client over an unreachable base — the write verbs never touch it.
        let client = SidecarClient::with_config(Config::new(
            "http://127.0.0.1:1",
            "tok",
            "ws",
            "control-engine",
        ));
        HostCtx::with_parts(client, caps, "ws")
    }

    async fn write(
        host: &HostCtx,
        fake: &CeFake,
        tool: &str,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, HostError> {
        let inst = EngineInstanceId::edge();
        super::dispatch_write(host, fake, &inst, tool, &input).await
    }

    #[tokio::test]
    async fn each_write_verb_self_checks_then_calls_the_trait_once() {
        let node = json!({ "uid": 1, "kind": "component" });
        let cases: Vec<(&str, serde_json::Value)> = vec![
            (
                "control-engine.add-node",
                json!({ "type": "test-math::add" }),
            ),
            (
                "control-engine.patch",
                json!({ "node": node, "values": { "in": 1 } }),
            ),
            (
                "control-engine.set-override",
                json!({ "node": node, "property": "in", "value": 2, "ttl_secs": 0 }),
            ),
            (
                "control-engine.clear-override",
                json!({ "node": node, "property": "in" }),
            ),
            (
                "control-engine.add-edge",
                json!({ "source": node, "source_property": "out", "target": node, "target_property": "in" }),
            ),
            ("control-engine.remove-node", json!({ "node": node })),
            (
                "control-engine.call-action",
                json!({ "node": node, "action": "reset", "params": {} }),
            ),
        ];

        for (tool, input) in cases {
            let verb = tool.strip_prefix("control-engine.").unwrap();
            let fake = CeFake::seeded();

            // Missing the verb's own cap → opaque Denied, and NO trait call.
            let denied = write(&host_granting(&[]), &fake, tool, input.clone()).await;
            assert!(
                matches!(denied, Err(HostError::Denied)),
                "{tool}: no cap → Denied, got {denied:?}"
            );
            assert_eq!(
                fake.calls.load(Ordering::SeqCst),
                0,
                "{tool}: deny before any trait call"
            );

            // With the cap granted → exactly one trait call.
            let host = host_granting(&[&format!("control-engine.{verb}")]);
            write(&host, &fake, tool, input)
                .await
                .expect("write dispatches");
            assert_eq!(
                fake.calls.load(Ordering::SeqCst),
                1,
                "{tool}: one trait call per dispatch"
            );
        }
    }

    #[tokio::test]
    async fn write_verb_rejects_missing_node_before_trait_call() {
        let fake = CeFake::seeded();
        let host = host_granting(&["control-engine.patch"]);
        // Well-formed cap grant, but no `node` → bad-input error, NO trait call.
        let err = write(
            &host,
            &fake,
            "control-engine.patch",
            json!({ "values": {} }),
        )
        .await
        .expect_err("patch without node is rejected");
        assert!(
            matches!(err, HostError::BadInput(_)),
            "missing node → bad input, got {err:?}"
        );
        assert_eq!(
            fake.calls.load(Ordering::SeqCst),
            0,
            "arg validation fails before any trait call"
        );
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
