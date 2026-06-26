//! The runtime **sidecar map** — the live native children on this node, keyed `(ws, ext_id)`
//! (native-tier scope). This is the ONLY place a live `Sidecar` (PID, child stdio) is held, and it
//! is **runtime-only**: it is never written to the store. The durable truth is the `Install` +
//! `native_status` records; this map is a cache the records could rebuild (the boot reconciler, a
//! follow-up). That is the stateless-extension rule applied to a process — the PID is disposable
//! motion, the record is state (§3.3/§3.4).
//!
//! Keyed by `(ws, ext_id)` so a lifecycle call resolves only *this workspace's* sidecar — a ws-B
//! `stop`/`restart`/`status` can never reach a ws-A child (workspace-first isolation, structural at
//! the map key, on top of the capability gate). Behind an `RwLock` + per-sidecar `Mutex` exactly
//! like the MCP `Registry`, so the host drives it with `&Node`.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use lb_supervisor::Sidecar;
use tokio::sync::Mutex as AsyncMutex;

/// The live native children on this node. One `Sidecar` per `(workspace, ext_id)`.
#[derive(Default)]
pub struct SidecarMap {
    // (ws, ext_id) → the live sidecar (async mutex: a tool call/lifecycle needs &mut on it).
    live: RwLock<HashMap<(String, String), Arc<AsyncMutex<Sidecar>>>>,
}

impl SidecarMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert (or replace) the live sidecar for `(ws, ext_id)`, returning the shared handle. A
    /// replace drops the previous handle — the caller has already stopped/killed it.
    pub(crate) fn insert(
        &self,
        ws: &str,
        ext_id: &str,
        sidecar: Sidecar,
    ) -> Arc<AsyncMutex<Sidecar>> {
        let handle = Arc::new(AsyncMutex::new(sidecar));
        self.live
            .write()
            .unwrap()
            .insert((ws.to_string(), ext_id.to_string()), handle.clone());
        handle
    }

    /// The live sidecar for `(ws, ext_id)` on this node, if running. `None` if not running here —
    /// which is exactly what a ws-B lookup of a ws-A sidecar returns (different key namespace).
    pub(crate) fn get(&self, ws: &str, ext_id: &str) -> Option<Arc<AsyncMutex<Sidecar>>> {
        self.live
            .read()
            .unwrap()
            .get(&(ws.to_string(), ext_id.to_string()))
            .cloned()
    }

    /// Remove the live sidecar for `(ws, ext_id)`, returning it if present (the caller shuts it down).
    pub(crate) fn remove(&self, ws: &str, ext_id: &str) -> Option<Arc<AsyncMutex<Sidecar>>> {
        self.live
            .write()
            .unwrap()
            .remove(&(ws.to_string(), ext_id.to_string()))
    }

    /// Is a native sidecar with this id running for this workspace on this node?
    pub fn is_running(&self, ws: &str, ext_id: &str) -> bool {
        self.live
            .read()
            .unwrap()
            .contains_key(&(ws.to_string(), ext_id.to_string()))
    }
}
