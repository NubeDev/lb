//! The MCP tool-call error. `Denied` deliberately carries no detail about which gate failed
//! or whether the tool exists — an unauthorized caller learns nothing (mcp scope).

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ToolError {
    /// Authorization failed (workspace isolation or missing capability). No further detail by
    /// design — does not reveal tool existence.
    #[error("denied")]
    Denied,
    /// The qualified tool name is malformed or not hosted here. Only reachable by an
    /// already-authorized caller.
    #[error("no such tool")]
    NotFound,
    /// The extension ran but returned an error or trapped.
    #[error("extension error: {0}")]
    Extension(String),
    /// The input was not valid for the tool.
    #[error("bad input: {0}")]
    BadInput(String),
    /// The call did not name a target node, and MORE THAN ONE node hosts the extension — so
    /// there is no single correct place to run it (routed-node-dispatch scope, #81).
    ///
    /// This replaces the pre-#81 behaviour, which was to route on a shared key that every host
    /// answered and keep whichever reply arrived FIRST — a silent coin flip that could provision
    /// the wrong physical box and still report success. Refusing is the whole point: there is no
    /// "reasonable default" node, because two nodes hosting one extension are two distinct
    /// physical things, not interchangeable replicas.
    ///
    /// Carries the candidate node ids so a caller (e.g. a supervisor's `Provisioner`) can react
    /// programmatically instead of parsing prose — hence structured data, not a message string.
    ///
    /// **Accepted trade-off (scope, "Capabilities"):** this lists fleet shape to a caller who
    /// already holds `mcp:<ext>.<tool>:call`. That is deliberate — such a caller could enumerate
    /// by targeting anyway, and an actionable error is the point. The line held is against
    /// UNAUTHORIZED enumeration, which is why `authorize` strictly precedes `resolve`: a capless
    /// caller gets `Denied` and never reaches this variant.
    #[error("ambiguous: extension {ext:?} is hosted by {} nodes; name a target node ({})", candidates.len(), candidates.join(", "))]
    Ambiguous {
        ext: String,
        /// The nodes hosting `ext`, sorted for a deterministic error (a `HashMap` iteration
        /// order would make the message vary run to run and the test flaky).
        candidates: Vec<String>,
    },
    /// A target node was named, but it is not currently reachable in this workspace.
    ///
    /// Deliberately a REFUSAL, never a queue and never a fallback to another node hosting the
    /// same extension — the fallback IS the misprovisioning bug (scope: "Honest failure when the
    /// node is gone"). A caller that wants deferral must use the outbox explicitly.
    ///
    /// Carries no detail beyond the node the caller itself named, so it cannot be used to
    /// enumerate a fleet.
    #[error("node {node:?} is not reachable")]
    NodeUnreachable { node: String },
    /// The target node is reachable but runs a build that predates routed dispatch, so it does
    /// not answer on its node-qualified key.
    ///
    /// Distinct from [`ToolError::NodeUnreachable`] on purpose: during a rolling upgrade an old
    /// node is *online and healthy*, and reporting it as unreachable would be a lie in exactly
    /// the scenario this scope claims to make honest (scope, open question 7). The distinction is
    /// only possible because the presence payload carries a `targeted_dispatch` flag — it cannot
    /// be retrofitted later, since an old node cannot be taught to announce anything new.
    #[error("node {node:?} does not support targeted dispatch (upgrade required)")]
    NodeTooOld { node: String },
}
