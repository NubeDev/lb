//! [`HttpSource`] — the real HTTP impl of `lb_host::Source`, the registry's fetch seam. It `GET`s a
//! signed artifact from a `registry-host` server and deserializes it. **It verifies nothing**: the
//! returned bytes are untrusted, and `lb_host::pull` runs them through `verify_artifact` before
//! caching (verify-before-cache, host-side). This file is the client peer of [`crate::server`].
//!
//! Why here and not in core `lb-host`: the host owns the `Source` *trait*; a concrete HTTP impl pulls
//! in `reqwest` (a full HTTP/TLS client), which has no place compiled into every node — an edge node
//! serving from cache, or fronting a UI, never needs it. So the client lives beside its server, the
//! same way `lb-role-ai-gateway` owns the `ModelAccess` impl while host owns that trait. Roles depend
//! on host, never the reverse.

use lb_host::{RegistryServiceError, Source};
use lb_registry::Artifact;

/// An HTTP `Source` pointed at a `registry-host` server's base URL (e.g. `http://registry:9000`).
/// Each `fetch` is a single `GET /artifacts/{ext_id}/{version}`.
pub struct HttpSource {
    base_url: String,
    client: reqwest::Client,
}

impl HttpSource {
    /// Build a source against `base_url` (no trailing slash needed). Reuses one `reqwest::Client` so
    /// connections pool across pulls.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }
}

impl Source for HttpSource {
    async fn fetch(&self, ext_id: &str, version: &str) -> Result<Artifact, RegistryServiceError> {
        let url = format!("{}/artifacts/{ext_id}/{version}", self.base_url);

        // A transport failure (server down / DNS / connection refused) and a `404` (no such version)
        // both collapse to `NotAvailable` — the `Source` contract makes them indistinguishable, and
        // it is exactly what lets an offline node fall through to its cache without leaking which case
        // it was. The error string carries the cause for the log, not for the caller to branch on.
        let resp =
            self.client.get(&url).send().await.map_err(|e| {
                RegistryServiceError::NotAvailable(format!("{ext_id}@{version}: {e}"))
            })?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(RegistryServiceError::NotAvailable(format!(
                "{ext_id}@{version}: not found"
            )));
        }
        let resp = resp
            .error_for_status()
            .map_err(|e| RegistryServiceError::NotAvailable(format!("{ext_id}@{version}: {e}")))?;

        // The body is the signed artifact, still UNTRUSTED. A decode failure is a malformed origin
        // response, not a verification failure — surface it as unavailable (the caller retries / the
        // signature check would reject it anyway).
        resp.json::<Artifact>()
            .await
            .map_err(|e| RegistryServiceError::NotAvailable(format!("{ext_id}@{version}: {e}")))
    }
}
