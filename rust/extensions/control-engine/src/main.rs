//! `control-engine` — the native (Tier-2) Control Engine bridge extension
//! (control-engine scope). A supervised OS child that holds the long-lived CE
//! REST/WS connection (the `rubix-ce` `ControlEngine` client) and serves the
//! caps-gated `control-engine.*` MCP surface over `Content-Length`-framed stdio
//! using the SAME `lb-supervisor` wire types the host uses — so the child↔host ABI
//! cannot drift (federation/echo-sidecar precedent).
//!
//! It is stateless (§3.4): it holds nothing durable. Each `call` carries an
//! `appliance` selector; the client is (re)built lazily and cached in-process, so a
//! kill + respawn loses nothing. This slice (S3) serves the two READ verbs against a
//! localhost CE; later slices add verbs + appliance-registry routing.
//!
//! Tools served (the NAME is the cap gate — `mcp:control-engine.<verb>:call`):
//!   - `control-engine.tree   {appliance, node?, depth?}` → `{nodes, edges}` (verbatim CE DTOs)
//!   - `control-engine.schema {appliance}`                → `{manifests}`    (the add-node palette)

mod args;
mod engine;
mod tools;

// The ONE sanctioned CE stub — compiled in only for the crate's own tests OR under
// the `ce-fake` build feature the host integration test uses (see ce_fake.rs). Never
// in a shipped binary's real call path.
#[cfg(any(test, feature = "ce-fake"))]
mod ce_fake;

use lb_supervisor::{read_frame, write_frame, CallParams, Method, Reply, Request};
use serde_json::Value;
use tokio::io::{stdin, stdout};

use engine::Registry;

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    let ext_id = std::env::var("LB_EXT_ID").unwrap_or_default();
    let registry = Registry::new();

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
            Method::Call => handle_call(&registry, &req).await,
        };

        let bytes = serde_json::to_vec(&reply).unwrap();
        if write_frame(&mut output, &bytes).await.is_err() {
            break;
        }
    }
}

/// Handle a `call`: parse the tool + input, resolve the appliance's CE client, and
/// dispatch to the verb. The `appliance` selector carries the CE base in S3 (S4
/// resolves it against the registry instead).
async fn handle_call(registry: &Registry, req: &Request) -> Reply {
    let params: CallParams = match serde_json::from_str(&req.params) {
        Ok(p) => p,
        Err(e) => return Reply::err(req.id, format!("bad params: {e}")),
    };
    let input: Value = match serde_json::from_str(&params.input) {
        Ok(v) => v,
        Err(e) => return Reply::err(req.id, format!("bad input json: {e}")),
    };

    // The shared envelope carries the appliance selector (S3 = the CE base host:port).
    let envelope: args::Envelope =
        serde_json::from_value(input.clone()).unwrap_or(args::Envelope {
            appliance: String::new(),
        });

    let bound = match bind(registry, &envelope.appliance) {
        Ok(b) => b,
        Err(e) => return Reply::err(req.id, format!("engine bind failed: {e}")),
    };

    match tools::dispatch(&*bound.engine, &bound.instance, &params.tool, &input).await {
        Ok(v) => Reply::ok(req.id, v.to_string()),
        Err(e) => Reply::err(req.id, e),
    }
}

/// Resolve the appliance to a bound CE client. Normally the real `rubix-ce` client
/// (via the registry). Under the `ce-fake` build feature AND `LB_CE_FAKE=1`, serve
/// the sanctioned in-memory stub instead — the host integration test path, so it can
/// drive the REAL supervisor + gate + stdio ABI without the C++ engine. OFF in a
/// shipped binary (the feature is off), so the real path always uses the real client.
fn bind(registry: &Registry, appliance: &str) -> Result<engine::Bound, String> {
    #[cfg(feature = "ce-fake")]
    {
        if std::env::var("LB_CE_FAKE").as_deref() == Ok("1") {
            return Ok(engine::Bound {
                engine: ce_fake::CeFake::seeded(),
                instance: rubix_ce::EngineInstanceId::edge(),
            });
        }
    }
    registry.get(appliance)
}

// ---------------------------------------------------------------------------------
// Crate-level unit tests: the dispatch layer's deny-before-call semantics + verbatim
// DTO shape, driven against the ONE sanctioned fake (`ce_fake`, with its call
// counter). These prove "0 trait calls before a denied call" at the dispatch seam
// (the host integration test proves the `Denied` at the call_tool boundary). No
// process, no store, no bus — just dispatch × the trait.
// ---------------------------------------------------------------------------------
#[cfg(test)]
mod dispatch_tests {
    use super::ce_fake::CeFake;
    use super::tools::dispatch;
    use rubix_ce::EngineInstanceId;
    use serde_json::json;
    use std::sync::atomic::Ordering;

    #[tokio::test]
    async fn tree_returns_seeded_graph_verbatim_and_counts_one_call() {
        let fake = CeFake::seeded();
        let inst = EngineInstanceId::edge();

        // Deny-before-call: a caller WITHOUT the cap never reaches dispatch (the
        // host gate stops it). We prove the counter is 0 until a call is dispatched.
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

        // Verbatim DTO shape: ComponentDto uses the keyed `uid`/`type`/`path` form.
        let nodes = out["nodes"].as_array().expect("nodes array");
        assert_eq!(nodes.len(), 1, "one seeded node: {out}");
        assert_eq!(nodes[0]["uid"], 1);
        assert_eq!(nodes[0]["type"], "test-math::add");
        assert_eq!(out["edges"].as_array().expect("edges array").len(), 0);

        // Exactly one trait call happened for one dispatched verb.
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
