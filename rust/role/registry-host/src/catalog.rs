//! The server's artifact origin — an in-memory `(ext_id, version) → Artifact` map (the cloud
//! catalog's stand-in). The server is a **dumb origin**: it holds whatever signed bytes a publisher
//! pushed and hands them back verbatim; it neither signs nor verifies. A durable backing
//! (SurrealDB / object store) and a publish endpoint are follow-ups — this slice ships the transport.
//!
//! `offline` models an unreachable origin for the offline test: with it set, every lookup misses, so
//! the server `404`s and the client sees `NotAvailable` — exactly as a real down server would look.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use lb_registry::Artifact;

/// The signed artifacts this registry-host serves, keyed by `(ext_id, version)`. Cloneable + shared
/// (`Arc`) so axum can hold it as router state across handlers. Interior `Mutex` so a test can flip
/// `offline` after the server is serving.
#[derive(Clone, Default)]
pub struct ArtifactStore {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    artifacts: HashMap<(String, String), Artifact>,
    offline: bool,
}

impl ArtifactStore {
    /// An origin seeded with `artifacts` (each keyed by its own `(ext_id, version)`).
    pub fn new(artifacts: Vec<Artifact>) -> Self {
        let store = Self::default();
        {
            let mut inner = store.inner.lock().unwrap();
            for a in artifacts {
                inner
                    .artifacts
                    .insert((a.ext_id.clone(), a.version.clone()), a);
            }
        }
        store
    }

    /// Flip the origin offline (every lookup misses → the server `404`s). Models an unreachable
    /// server without tearing down the socket, so the offline-cached path can be exercised
    /// deterministically.
    pub fn set_offline(&self, offline: bool) {
        self.inner.lock().unwrap().offline = offline;
    }

    /// Look up the artifact for `(ext_id, version)`. `None` if the origin lacks it OR is offline —
    /// the two are deliberately indistinguishable (the `Source` contract: an offline origin and a
    /// missing version look the same to the client).
    pub(crate) fn get(&self, ext_id: &str, version: &str) -> Option<Artifact> {
        let inner = self.inner.lock().unwrap();
        if inner.offline {
            return None;
        }
        inner
            .artifacts
            .get(&(ext_id.to_string(), version.to_string()))
            .cloned()
    }
}
