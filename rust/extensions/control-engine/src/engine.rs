//! The per-appliance CE client cache (control-engine scope: hold the
//! `Arc<dyn ControlEngine>` per bound CE, lazily constructed from a `base`
//! host:port via `CeRestClient`, cached by appliance id).
//!
//! The extension is stateless (§3.4): this cache is a pure in-memory connection
//! pool, not durable state — a kill + respawn rebuilds it lazily from the same
//! `appliance` selector on the next call. It holds no graph, no session, no token.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rubix_ce::{CeRestClient, ControlEngine, EngineConfig, EngineInstanceId};

use crate::args::base_of;

/// A trait-object CE client and the instance id its UIDs are keyed against.
#[derive(Clone)]
pub struct Bound {
    /// The narrow client contract — every verb is a thin map onto one method.
    pub engine: Arc<dyn ControlEngine>,
    /// The engine instance the client is bound to (keys every `NodeRef`).
    pub instance: EngineInstanceId,
}

/// Lazily builds + caches one CE client per appliance selector. `Send + Sync` so it
/// can be shared across the async control loop.
#[derive(Default)]
pub struct Registry {
    /// Keyed by the raw `appliance` selector string (S3 = the base host:port; S4
    /// swaps in the registry id → node/base resolution).
    clients: Mutex<HashMap<String, Bound>>,
}

impl Registry {
    /// A fresh, empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get (or lazily construct) the CE client for `appliance`. The instance id is
    /// derived from the base so each distinct CE keys its own UID pools.
    ///
    /// # Errors
    /// Returns the CE client build error string if the config is invalid.
    pub fn get(&self, appliance: &str) -> Result<Bound, String> {
        if let Some(b) = self.clients.lock().unwrap().get(appliance) {
            return Ok(b.clone());
        }
        let (host, port) = base_of(appliance);
        let instance = EngineInstanceId::new(format!("{host}:{port}"));
        let cfg = EngineConfig::new(host, port, instance.clone());
        let client = CeRestClient::new(cfg).map_err(|e| e.to_string())?;
        let bound = Bound {
            engine: Arc::new(client),
            instance,
        };
        self.clients
            .lock()
            .unwrap()
            .insert(appliance.to_string(), bound.clone());
        Ok(bound)
    }
}
