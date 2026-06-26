//! One workspace's binding for the background driver: which workspace to drive, the service
//! principal to drive it as, and the channel job-progress streams to. The driver runs one of these
//! per workspace per tick — keeping them as a list (not a single global) is what makes the loop
//! workspace-isolated: every `relay_outbox` / `react_to_approvals` call selects its binding's `ws`,
//! so one binding can never touch another's effects or approvals (the hard wall, §7).
//!
//! The principal is workspace-scoped and holds exactly the workflow caps (`mcp:workflow.start_job:call`
//! for the reactor; the relay needs none — it is a host service over the durable set). It is the
//! service identity the unattended loop acts as, the same one the webhook ingress uses.

use std::sync::Arc;

use lb_auth::Principal;

/// A workspace the driver services each tick.
#[derive(Clone)]
pub struct WorkflowBinding {
    /// The workspace whose outbox + approvals this binding drives.
    pub ws: String,
    /// The service principal the unattended loop acts as (workspace-scoped, holds the workflow caps).
    pub principal: Arc<Principal>,
    /// The channel the reactor streams job progress to (motion).
    pub channel: String,
}

impl WorkflowBinding {
    pub fn new(ws: impl Into<String>, principal: Principal, channel: impl Into<String>) -> Self {
        Self {
            ws: ws.into(),
            principal: Arc::new(principal),
            channel: channel.into(),
        }
    }
}
