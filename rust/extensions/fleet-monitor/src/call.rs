//! Tool dispatch for the `fleet-monitor` sidecar (FILE-LAYOUT: the stdio loop is `main.rs`, the tool
//! handling is here). One tool this slice — `fleet.summary` — proving the BACKEND half is a real,
//! reachable native MCP tool with its own PID. It is stateless: the reply is a pure function of the
//! request + the injected workspace identity, so a kill + respawn loses nothing (§3.4).
//!
//! `fleet.summary` returns a small JSON object tagged with the workspace the host spawned us in (read
//! from `LB_EXT_WS`), proving the injected identity reached the child — the same identity-injection
//! proof `echo-sidecar` makes, here behind a fleet-shaped verb. The UI does NOT bind to this verb
//! (the frozen widget/bridge contract is series-read-only); it exists to prove a native extension can
//! ship a real backend tool alongside its federated frontend in ONE folder.

use lb_supervisor::{CallParams, Reply, Request};

/// Handle a `call`: parse the tool + input, dispatch. Unknown tools are an explicit error (never a
/// silent success). `ws` is the injected workspace identity attached to the reply.
pub fn handle(req: &Request, ws: &str) -> Reply {
    let params: CallParams = match serde_json::from_str(&req.params) {
        Ok(p) => p,
        Err(e) => return Reply::err(req.id, format!("bad params: {e}")),
    };

    match params.tool.as_str() {
        "fleet.summary" => Reply::ok(req.id, summary_json(ws)),
        other => Reply::err(req.id, format!("unknown tool: {other}")),
    }
}

/// The `fleet.summary` body — a stateless, workspace-tagged JSON snapshot. The counts are derived,
/// not stored (the sidecar holds nothing durable); a real deployment would read them through the host
/// from the store, but this slice proves the *reachability* of a native tool, not fleet analytics.
fn summary_json(ws: &str) -> String {
    format!(r#"{{"ok":true,"ws":"{ws}","node":"fleet-monitor","tier":"native"}}"#)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_supervisor::Method;
    use serde_json::Value;

    fn call_req(tool: &str) -> Request {
        Request {
            id: 1,
            method: Method::Call,
            params: format!(r#"{{"tool":"{tool}","input":"{{}}"}}"#),
        }
    }

    #[test]
    fn fleet_summary_is_tagged_with_the_injected_workspace() {
        let reply = handle(&call_req("fleet.summary"), "acme");
        let v: Value = serde_json::to_value(&reply).unwrap();
        // The supervisor `Reply::ok` carries the result JSON string in `result`; assert ws round-trips.
        let result = v.get("result").and_then(|r| r.as_str()).expect("ok result");
        let parsed: Value = serde_json::from_str(result).expect("result is JSON");
        assert_eq!(parsed["ws"], "acme");
        assert_eq!(parsed["tier"], "native");
        assert_eq!(parsed["ok"], true);
    }

    #[test]
    fn unknown_tool_is_an_explicit_error() {
        let reply = handle(&call_req("fleet.delete"), "acme");
        let v: Value = serde_json::to_value(&reply).unwrap();
        // An unknown tool yields an `err` reply (never a silent ok) — assert the error path.
        assert!(
            v.get("error").is_some(),
            "unknown tool must be an error reply, got {v}"
        );
    }

    #[test]
    fn bad_params_is_an_error_not_a_panic() {
        let req = Request {
            id: 2,
            method: Method::Call,
            params: "not json".into(),
        };
        let reply = handle(&req, "acme");
        let v: Value = serde_json::to_value(&reply).unwrap();
        assert!(v.get("error").is_some());
    }
}
