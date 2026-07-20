//! The **local** transport (operator-cli scope, decision #2 + the offline posture): embed
//! `Node::boot()` and call `lb_host::call_tool` in-process — no daemon, no network, fully offline.
//! This IS the edge/solo posture; the same crates run everywhere.
//!
//! The principal is minted with the **same `dev_claims` claim set** the gateway's `/login` issues,
//! scoped by `-w` (the local-vs-remote parity decision) — so local is NOT silently more privileged
//! than a real login. The workspace the principal is scoped to IS the wall (`call_tool` gate 1): a
//! local `-w acme` principal cannot reach outside `acme`, exactly like a remote `acme` token.
//!
//! A local DENY (`ToolError::Denied`) maps to the same `DENIED  mcp:<tool>:call` a remote `403` does —
//! so the two modes produce identical honest output (the parity test: local denies the same verbs a
//! member token would).

use std::sync::Arc;

use lb_auth::Principal;
use lb_host::Node;
use lb_mcp::ToolError;
use serde_json::Value;

use crate::error::{CliError, CliResult};
use crate::header::Header;

use super::Transport;

/// An in-process node + a minted principal scoped to one workspace. Booting the node opens the
/// embedded store + bus (in-memory unless `LB_STORE_PATH` points at a durable one) — so a local run
/// works with no gateway reachable at all.
pub struct Local {
    node: Arc<Node>,
    principal: Principal,
    workspace: String,
}

impl Local {
    /// Boot a solo node and mint a `dev_claims`-shaped principal for `(user, workspace)`. The claim
    /// set is the gateway's own `dev_claims` (parity), realized as a `Principal::routed` — the honest
    /// in-process co-trust constructor (the node runs the principal in-process; the workspace-scoped
    /// wall still holds). `now`/`ttl` mirror the dev-login so a local principal expires like a session.
    pub async fn boot(user: &str, workspace: &str) -> CliResult<Self> {
        let node = Node::boot()
            .await
            .map_err(|e| CliError::Other(format!("local node boot failed: {e}")))?;
        Ok(Self::over(Arc::new(node), user, workspace))
    }

    /// Build a local transport over an already-booted `node` (the tests share one node between two
    /// principals to prove workspace isolation on a single store). The principal carries the same caps
    /// `dev_claims` mints, scoped to `workspace`.
    pub fn over(node: Arc<Node>, user: &str, workspace: &str) -> Self {
        // Reuse the gateway's dev_claims so local == a real login (parity). The clock values do not
        // matter for an in-process principal (there is no expiry re-check on `routed`), but we pass
        // the real dev caps so the DENY surface matches a member token exactly.
        let claims = lb_role_gateway::dev_claims(user, workspace, 0, u64::MAX);
        let principal = Principal::routed(claims.sub, claims.ws, claims.caps);
        Self {
            node,
            principal,
            workspace: workspace.to_string(),
        }
    }

    /// The in-process node (the tests seed the store through it, over the real write path).
    pub fn node(&self) -> &Arc<Node> {
        &self.node
    }

    /// The minted principal (the tests seed inbox items as this principal).
    pub fn principal(&self) -> &Principal {
        &self.principal
    }
}

impl Transport for Local {
    fn header(&self) -> Header {
        Header::new(
            self.principal.ws(),
            self.principal.sub(),
            // dev_claims always mints Member; read it off the principal rather than assuming.
            self.principal.role(),
            true,
        )
    }

    fn caps(&self) -> Vec<String> {
        self.principal.caps().to_vec()
    }

    async fn call(&self, tool: &str, args: Value) -> CliResult<Value> {
        let input = if args.is_null() {
            "{}".to_string()
        } else {
            args.to_string()
        };
        // The SAME chokepoint the gateway reaches — workspace-first, then `mcp:<tool>:call`, then
        // dispatch. The workspace is the principal's (never caller-supplied): `-w` scoped it, and it
        // cannot be overridden here.
        let out =
            lb_host::call_tool(&self.node, &self.principal, &self.workspace, tool, &input).await;
        match out {
            Ok(s) => Ok(serde_json::from_str(&s).unwrap_or(Value::String(s))),
            // A local deny is the same honest deny a remote 403 is — surface it identically.
            Err(ToolError::Denied) => Err(CliError::Denied {
                tool: tool.to_string(),
            }),
            Err(ToolError::NotFound) => Err(CliError::BadInput(format!("no such tool: {tool}"))),
            Err(ToolError::BadInput(msg)) => Err(CliError::BadInput(msg)),
            Err(ToolError::Extension(msg)) => Err(CliError::Other(format!("tool error: {msg}"))),
            // Routed-dispatch failures (#81). Their `Display` is already written for a human — it
            // names the candidate nodes (`Ambiguous`) or the node that could not be reached — so it
            // is surfaced verbatim rather than flattened into a generic message. `Ambiguous` in
            // particular tells a CLI user exactly what to do next: name a target node.
            Err(e @ (ToolError::Ambiguous { .. }
            | ToolError::NodeUnreachable { .. }
            | ToolError::NodeTooOld { .. })) => Err(CliError::Other(e.to_string())),
        }
    }
}
