//! `reconcile` — the boot reconciler (lifecycle-management scope). On node start, the node calls this
//! once to re-apply the durable intent `enabled ∧ (should-run)`: every **enabled** install that is
//! not currently running is planned for (re)start; every **disabled** install is skipped, so a
//! disabled extension does **not** silently come back after a restart — the load-bearing distinction
//! `enable`/`disable` exists for.
//!
//! It returns a **plan** (`ReconcilePlan`) rather than spawning directly: the actual native respawn
//! needs the `Launcher` + the install dir the node binary owns, so the host verb stays testable
//! headlessly and symmetric (the resolved open question: a host `reconcile` verb the node calls on
//! start). Reconcile reads only durable records + the live `SidecarMap` and is idempotent against
//! them — it never double-plans an already-running extension, so it can't fight an in-flight op.
//!
//! Not capability-gated: it is a node-boot operation, not a caller verb (like the workflow driver).
//! It is workspace-scoped — it reconciles exactly the `ws` it is given.

use lb_assets::{list_installs, Tier};
use serde::{Deserialize, Serialize};

use super::error::ExtError;
use crate::boot::Node;

/// What reconcile decided for one extension on boot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconcileAction {
    pub ext: String,
    pub tier: String,
    /// `true` → the node should (re)start it; `false` → leave it (disabled, or already running).
    pub start: bool,
    /// Why, for the boot log: `"start"` / `"disabled"` / `"already-running"`.
    pub reason: String,
}

/// The reconcile plan for a workspace — one action per installed extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconcilePlan {
    pub ws: String,
    pub actions: Vec<ReconcileAction>,
}

/// Compute the boot plan for workspace `ws`: start every enabled, not-currently-running install;
/// skip disabled ones and already-running ones. Idempotent (driven by durable + live state).
pub async fn reconcile(node: &Node, ws: &str) -> Result<ReconcilePlan, ExtError> {
    let installs = list_installs(&node.store, ws).await?;
    let mut actions = Vec::with_capacity(installs.len());
    for install in &installs {
        let running = match install.tier {
            Tier::Native => node.sidecars.is_running(ws, &install.ext_id),
            Tier::Wasm => false, // wasm "start" = load; reconcile plans the load for enabled ones.
        };
        let (start, reason) = if !install.enabled {
            (false, "disabled")
        } else if running {
            (false, "already-running")
        } else {
            (true, "start")
        };
        let tier = match install.tier {
            Tier::Wasm => "wasm",
            Tier::Native => "native",
        };
        actions.push(ReconcileAction {
            ext: install.ext_id.clone(),
            tier: tier.to_string(),
            start,
            reason: reason.to_string(),
        });
    }
    Ok(ReconcilePlan {
        ws: ws.to_string(),
        actions,
    })
}
