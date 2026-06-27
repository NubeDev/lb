//! The server's artifact origin — an in-memory `(ext_id, version) → Artifact` map (the cloud
//! catalog's stand-in). The server is a **dumb origin**: it holds whatever signed bytes a publisher
//! pushed and hands them back verbatim; it neither signs nor verifies. A durable backing
//! (SurrealDB / object store) and a publish endpoint are follow-ups — this slice ships the transport.
//!
//! `offline` models an unreachable origin for the offline test: with it set, every lookup misses, so
//! the server `404`s and the client sees `NotAvailable` — exactly as a real down server would look.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use lb_registry::{verify_artifact, Artifact, RegistryError, TrustedKeys};

/// The signed artifacts this registry-host serves, keyed by `(ext_id, version)`. Cloneable + shared
/// (`Arc`) so axum can hold it as router state across handlers. Interior `Mutex` so a test can flip
/// `offline` after the server is serving.
///
/// It now also carries the **publisher allow-list** (`TrustedKeys`) so the publish endpoint can
/// `verify_artifact` an upload **before** storing it (authenticity gate before authority,
/// lifecycle-management scope). The read path stays dumb (self-authenticating artifacts); only the
/// *write* path verifies, so an unsigned/foreign artifact never reaches the catalog.
#[derive(Clone, Default)]
pub struct ArtifactStore {
    inner: Arc<Mutex<Inner>>,
    trusted: Arc<TrustedKeys>,
}

#[derive(Default)]
struct Inner {
    artifacts: HashMap<(String, String), Artifact>,
    offline: bool,
}

impl ArtifactStore {
    /// An origin seeded with `artifacts` (each keyed by its own `(ext_id, version)`).
    pub fn new(artifacts: Vec<Artifact>) -> Self {
        Self::with_trusted(artifacts, TrustedKeys::new())
    }

    /// An origin seeded with `artifacts` and a publisher allow-list the **publish** endpoint verifies
    /// uploads against. The read path ignores `trusted` (artifacts are self-authenticating); the
    /// write path requires an upload to verify under it before storing.
    pub fn with_trusted(artifacts: Vec<Artifact>, trusted: TrustedKeys) -> Self {
        let store = Self {
            inner: Arc::new(Mutex::new(Inner::default())),
            trusted: Arc::new(trusted),
        };
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

    /// Publish `artifact`: **verify it against the allow-list FIRST**, then store it (the
    /// authenticity-before-authority gate). A tampered/unsigned/foreign-key artifact is rejected with
    /// [`RegistryError::Unverified`] and **nothing is stored**. Idempotent on `(ext_id, version)` —
    /// re-publishing the same verified bytes upserts the same entry (no duplicate catalog rows).
    pub fn publish(&self, artifact: Artifact) -> Result<(), RegistryError> {
        let verified = verify_artifact(artifact, &self.trusted)?; // verify BEFORE store.
        let a = verified.into_artifact();
        self.inner
            .lock()
            .unwrap()
            .artifacts
            .insert((a.ext_id.clone(), a.version.clone()), a);
        Ok(())
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
    pub fn get(&self, ext_id: &str, version: &str) -> Option<Artifact> {
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
