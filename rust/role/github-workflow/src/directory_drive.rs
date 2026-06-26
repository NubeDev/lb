//! The **directory-backed** driver: instead of a fixed binding list, re-read the host's workflow
//! directory (`enabled_workspaces`) **each tick** and build the bindings from it. This is what makes a
//! workspace onboardable **without a restart** (workflow-driver scope's "dynamic workspace set"): a
//! `register_workspace` call lands a durable row, and the next tick picks it up; a `deregister`
//! disables the row, and the next tick drops it. The directory is the source of truth, re-read every
//! tick — the same durable-scan discipline as the relay/reactor, lifted to the *set* of workspaces.
//!
//! The crate has no caps/identity knowledge, so the service principal is supplied by an injected
//! `principal_for(ws) -> Principal` (the binary mints it from caps; a test supplies a granted one).
//! Everything else — the per-binding reactor+relay passes, the per-ws error isolation, the injected
//! clock — is exactly `drive_once`, which this composes.

use lb_auth::Principal;
use lb_host::{enabled_workspaces, Node, Target};

use crate::binding::WorkflowBinding;
use crate::drive::{drive_once, Tick};

/// Run **one tick over the current directory** at logical time `now`: read every enabled workspace,
/// build a binding for each (minting its principal via `principal_for`), then drive them all (reactor
/// then relay) through `target`. A directory read error is reported to `on_error` and the tick is
/// skipped (the next one re-reads — never wedged). Returns the tally.
pub async fn drive_directory_once<T, P, F>(
    node: &Node,
    target: &T,
    now: u64,
    mut principal_for: P,
    mut on_error: F,
) -> Tick
where
    T: Target,
    P: FnMut(&str) -> Principal,
    F: FnMut(&str, String),
{
    let entries = match enabled_workspaces(&node.store).await {
        Ok(e) => e,
        Err(e) => {
            on_error("_directory", format!("read: {e}"));
            return Tick::default();
        }
    };
    let bindings: Vec<WorkflowBinding> = entries
        .into_iter()
        .map(|e| WorkflowBinding::new(e.ws.clone(), principal_for(&e.ws), e.channel))
        .collect();
    drive_once(node, &bindings, target, now, on_error).await
}

/// Run the directory-backed driver **forever**, ticking every `interval`. Re-reads the directory each
/// tick, so workspaces registered/deregistered at runtime take effect on the next tick — no restart.
/// `clock` supplies `now`; errors go to `on_error` and never stop the loop.
pub async fn run_directory_loop<T, P, C, F>(
    node: &Node,
    target: T,
    interval: std::time::Duration,
    mut principal_for: P,
    mut clock: C,
    mut on_error: F,
) where
    T: Target,
    P: FnMut(&str) -> Principal,
    C: FnMut() -> u64,
    F: FnMut(&str, String),
{
    loop {
        let now = clock();
        drive_directory_once(node, &target, now, &mut principal_for, &mut on_error).await;
        tokio::time::sleep(interval).await;
    }
}
