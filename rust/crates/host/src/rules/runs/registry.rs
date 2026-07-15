//! The live rule-run registry — `(ws, run_id)` → the run's shared [`RunControl`]
//! (long-running-rules-scope). Runtime-only motion (the durable truth is the `job:{id}` record,
//! the `sidecars: Arc<SidecarMap>` precedent): a control verb sets intent on the live flag when
//! the run is on this node, and acts on the record when it is not (`live:false` — an orphan).
//! Entries are inserted by the worker at spawn and removed when the eval settles, so `is_live`
//! is an honest "a thread is evaluating this run right now".

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use lb_rules::RunControl;

/// The node-wide map of live rule runs. Hangs off [`Node`](crate::boot::Node) (shared `Arc`) so
/// the start/control verbs and the worker see one source of truth.
#[derive(Default)]
pub struct RuleRunMap {
    inner: Mutex<HashMap<(String, String), Arc<RunControl>>>,
}

impl RuleRunMap {
    /// Register a run as live; returns its fresh control. Replaces any stale entry (a re-attach
    /// after a worker died un-deregistered).
    pub fn insert(&self, ws: &str, run_id: &str) -> Arc<RunControl> {
        let control = Arc::new(RunControl::default());
        self.inner
            .lock()
            .expect("rule-run registry lock")
            .insert((ws.to_string(), run_id.to_string()), control.clone());
        control
    }

    /// The live run's control, if this node is evaluating it.
    pub fn get(&self, ws: &str, run_id: &str) -> Option<Arc<RunControl>> {
        self.inner
            .lock()
            .expect("rule-run registry lock")
            .get(&(ws.to_string(), run_id.to_string()))
            .cloned()
    }

    /// Whether the run is live on this node.
    pub fn is_live(&self, ws: &str, run_id: &str) -> bool {
        self.get(ws, run_id).is_some()
    }

    /// Deregister a settled run (worker exit path — success, pause, cancel, or failure alike).
    pub fn remove(&self, ws: &str, run_id: &str) {
        self.inner
            .lock()
            .expect("rule-run registry lock")
            .remove(&(ws.to_string(), run_id.to_string()));
    }
}
