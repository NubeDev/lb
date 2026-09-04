//! Shared real-infra fixture for the `series.producer.health` suites (series-observability scope,
//! slice D), `#[path]`-included by `series_producer_health_test.rs` (the contract) and
//! `series_producer_health_caps_test.rs` (the mandatory deny + isolation categories).
//!
//! Nothing here is a mock. [`HealthReporter`] implements `LocalDispatch` — the SAME trait a wasm
//! instance and a native sidecar implement, and the one `routed_host_entry_test.rs` uses to prove
//! routed dispatch — and it is reached through the production registry and the real `call_tool`
//! chokepoint. It is a real implementor AT the seam, not a stand-in for the host code under test.
//!
//! What these suites deliberately do NOT claim to prove is that a separately-built, separately
//! published extension answers the convention. That is proven live against `modbus` on a running
//! node and recorded in the session doc — a contract with no out-of-tree implementor is exactly what
//! the scope refused to ship the first time.

#![allow(dead_code)] // each including suite uses a subset

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node, PRODUCER_HEALTH_TOOL};
use lb_ingest::{Qos, Sample};
use lb_mcp::{ToolDescriptor, ToolError};
use lb_runtime::{CallContext, LocalDispatch, RuntimeError};
use serde_json::{json, Value};
use tokio::sync::Mutex;

pub const VERB: &str = "series.producer.health";
pub const VERB_CAP: &str = "mcp:series.producer.health:call";
pub const SERIES: &str = "plant-a.chiller-1.current-l1";

/// A real extension-side implementor of the convention. `reply` is the raw JSON body it answers
/// with, so a suite can drive the well-formed path AND the malformed one through identical plumbing.
/// With `echo_args` it reflects the arguments it was handed into `details`, which is what proves the
/// host passes the producer its OWN stream id (the leaf) rather than the rooted form it never saw.
pub struct HealthReporter {
    pub reply: String,
    pub echo_args: bool,
}

#[async_trait::async_trait]
impl LocalDispatch for HealthReporter {
    async fn call_tool(
        &mut self,
        _ws: &str,
        tool: &str,
        input_json: &str,
        _ctx: Option<CallContext>,
    ) -> Result<String, RuntimeError> {
        if tool != PRODUCER_HEALTH_TOOL {
            return Err(RuntimeError::Tool(format!("unknown tool: {tool}")));
        }
        if !self.echo_args {
            return Ok(self.reply.clone());
        }
        let args: Value = serde_json::from_str(input_json).unwrap_or(json!({}));
        let mut body: Value = serde_json::from_str(&self.reply).expect("reply is json");
        body["details"] = json!([
            { "label": "echo.producer", "value": args["producer"].as_str().unwrap_or("<none>") },
            { "label": "echo.series",   "value": args["series"].as_str().unwrap_or("<none>") },
        ]);
        Ok(body.to_string())
    }
}

pub fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// A principal holding the verb plus whatever per-extension caps a case needs.
pub fn admin(ws: &str, extra: &[&str]) -> Principal {
    let mut caps = vec![VERB_CAP.to_string()];
    caps.extend(extra.iter().map(|s| s.to_string()));
    let refs: Vec<&str> = caps.iter().map(String::as_str).collect();
    principal("user:test", ws, &refs)
}

/// Register `ext` as a real registry entry declaring the health convention.
pub fn register_reporter(node: &Node, ext: &str, reply: &str, echo_args: bool) {
    node.registry.register_local_dispatch(
        ext,
        vec![ToolDescriptor::name_only(PRODUCER_HEALTH_TOOL)],
        Arc::new(Mutex::new(HealthReporter {
            reply: reply.to_string(),
            echo_args,
        })),
    );
}

/// Register `ext` declaring some OTHER tool — installed and reachable, but opting out of the
/// convention. This is the honest majority case and must not read as broken.
pub fn register_silent(node: &Node, ext: &str) {
    node.registry.register_local_dispatch(
        ext,
        vec![ToolDescriptor::name_only("network.status")],
        Arc::new(Mutex::new(HealthReporter {
            reply: "{}".into(),
            echo_args: false,
        })),
    );
}

/// Commit one sample for `producer` on [`SERIES`] in `ws`, through the real write → drain path.
///
/// `lb_ingest::write` is used directly (not the `ingest.write` verb) precisely because the verb
/// would ROOT the producer at the calling principal — here the stored producer string IS the input
/// under test, so it is written verbatim.
pub async fn seed(node: &Node, ws: &str, producer: &str, seq: u64) {
    let s = Sample {
        series: SERIES.into(),
        producer: producer.into(),
        ts: 1_700_000_000_000 + seq,
        seq,
        payload: json!({ "v": 12.5 }),
        labels: json!({}),
        qos: Qos::BestEffort,
    };
    lb_ingest::commit_direct(&node.store, ws, &[s])
        .await
        .unwrap();
}

pub async fn health(node: &Arc<Node>, p: &Principal, ws: &str) -> Result<Value, ToolError> {
    let body = call_tool(node, p, ws, VERB, &json!({ "series": SERIES }).to_string()).await?;
    Ok(serde_json::from_str(&body).expect("verb returns json"))
}

/// The one row for `producer`, or a panic naming what WAS returned — a silently-missing row is the
/// failure mode a `.find().is_none()` assertion would hide.
pub fn row<'a>(out: &'a Value, producer: &str) -> &'a Value {
    out["producers"]
        .as_array()
        .expect("producers is an array")
        .iter()
        .find(|r| r["producer"] == producer)
        .unwrap_or_else(|| panic!("no row for {producer} in {out}"))
}
