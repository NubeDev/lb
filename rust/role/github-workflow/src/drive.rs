//! The background **driver**: turn the host's durable-scan verbs (`react_to_approvals` + the outbox
//! `relay_outbox`) into a running service. The host owns the verbs (each a stateless function over a
//! durable set); this owns the *loop* that calls them on a tick — the same split as the gateway role
//! (the host owns the MCP pipeline; the gateway owns the HTTP server).
//!
//! One **tick** does, for each workspace binding, in order:
//!   1. **reactor pass** — `react_to_approvals`: every approval that landed `Approved` and has not yet
//!      started auto-starts its coding job, queuing the PR effect through the outbox.
//!   2. **relay pass** — `relay_outbox`: every *due* effect (past its backoff gate) is delivered
//!      through the `Target`, marked delivered / failed / dead-lettered.
//!
//! Reactor-before-relay so a freshly-approved job's PR goes out in the *same* tick, not the next one.
//!
//! `now` is **injected** as a clock closure — the no-wall-clock rule keeps time out of the *core
//! crates* (testing §3), and the binary is the legitimate boundary where wall-clock enters: it passes
//! `|| unix_seconds()`, a test passes a deterministic counter. A tick never fails the loop: a per-ws
//! error (a store blip) is logged via the `on_error` sink and the loop continues — the durable set is
//! the source of truth, so the next tick simply re-reads it (never lost).
//!
//! Workspace isolation holds structurally: each binding's calls select its own `ws`, so a tick for
//! ws-A can neither deliver ws-B's effects nor start ws-B's jobs.

use lb_host::{react_to_approvals, relay_outbox, Node, ReactorPass, RelayPass, Target};

use crate::binding::WorkflowBinding;

/// The tally of one full tick across all bindings — the sum of every binding's reactor + relay pass.
/// Returned so a caller (or a test) can assert progress without scraping logs.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Tick {
    pub started: usize,
    pub delivered: usize,
    pub failed: usize,
    pub dead_lettered: usize,
}

impl Tick {
    fn add_reactor(&mut self, p: ReactorPass) {
        self.started += p.started;
    }
    fn add_relay(&mut self, p: RelayPass) {
        self.delivered += p.delivered;
        self.failed += p.failed;
        self.dead_lettered += p.dead_lettered;
    }
}

/// Run **one tick** at logical time `now`: for each binding, a reactor pass then a relay pass
/// (delivering through `target`). A per-binding error is reported to `on_error` and skipped — the tick
/// still services the other workspaces, and the next tick re-reads the durable set. Returns the tally.
pub async fn drive_once<T, F>(
    node: &Node,
    bindings: &[WorkflowBinding],
    target: &T,
    now: u64,
    mut on_error: F,
) -> Tick
where
    T: Target,
    F: FnMut(&str, String),
{
    let mut tick = Tick::default();
    for b in bindings {
        // 1. Reactor: auto-start jobs whose approval has landed (queues their PR effects).
        match react_to_approvals(node, &b.principal, &b.ws, &b.channel, now).await {
            Ok(p) => tick.add_reactor(p),
            Err(e) => on_error(&b.ws, format!("reactor: {e}")),
        }
        // 2. Relay: deliver every due effect through the target (at-least-once + backoff/dead-letter).
        match relay_outbox(&node.store, &b.ws, target, now).await {
            Ok(p) => tick.add_relay(p),
            Err(e) => on_error(&b.ws, format!("relay: {e}")),
        }
    }
    tick
}

/// Run the driver **forever**, ticking every `interval`. `clock` supplies `now` each tick (the binary
/// passes wall-clock seconds; a test passes a counter). Errors are reported to `on_error` and never
/// stop the loop. This is the long-running service the `node` binary spawns when the github-workflow
/// role is configured; it returns only if the process ends.
pub async fn run_workflow_loop<T, C, F>(
    node: &Node,
    bindings: Vec<WorkflowBinding>,
    target: T,
    interval: std::time::Duration,
    mut clock: C,
    mut on_error: F,
) where
    T: Target,
    C: FnMut() -> u64,
    F: FnMut(&str, String),
{
    loop {
        let now = clock();
        drive_once(node, &bindings, &target, now, &mut on_error).await;
        tokio::time::sleep(interval).await;
    }
}
