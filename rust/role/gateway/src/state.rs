//! The gateway's shared state: the in-process node it fronts plus the **node signing key** it
//! mints and verifies session tokens with. The gateway IS a node (symmetric nodes, §3.1) — it
//! just also exposes an HTTP/SSE surface so a *browser* can reach it (README §6.13). It adds no
//! authority of its own; every route reads the caller's **bearer token**, verifies it with this
//! key (`session::authenticate`), and forwards to `lb_host::*` with the **verified principal** —
//! so the SAME capability check guards the browser as guards every other caller (capability-first,
//! §3.5), and the workspace comes from the *token*, never the request (the hard wall, §7).
//!
//! The demo principal is gone (collaboration scope, slice 1): `login` issues a real signed token
//! and every other route verifies it. The credential check behind `login` is a dev-login for now
//! (pick a principal); the *token path* is real (mint + verify). A real IdP plugs in behind the
//! same `verify` seam later — `Non-goals` in the scope.

use std::sync::Arc;

use lb_auth::SigningKey;
use lb_host::Node;
use lb_registry::TrustedKeys;

/// The live node + the node's token-signing key, shared across handlers (`Arc` so axum can clone
/// it into each request). The key never leaves the node — the UI only ever holds the *issued*
/// token (scope: "Secrets").
#[derive(Clone)]
pub struct Gateway {
    pub node: Arc<Node>,
    /// The node's Ed25519 signing key — `login` mints with it, every route verifies with it.
    pub key: Arc<SigningKey>,
    /// The logical "now" (unix seconds) used for mint `iat`/`exp` and verify expiry. Injected so
    /// tests are deterministic (testing §3 — no wall-clock); `boot` seeds it from the real clock.
    pub now: u64,
    /// The publisher allow-list the `POST /extensions` upload verifies an artifact against BEFORE
    /// storing it (verify-before-store, lifecycle-management scope). Trust is environment, never the
    /// upload body — an attacker cannot self-trust. S7-first: an empty dev fixture in production
    /// (no publishers wired yet), seeded by tests; durable storage + rotation are deferred (registry
    /// scope open questions). Held behind `Arc` so axum clones it cheaply per request.
    pub trusted: Arc<TrustedKeys>,
    /// The directory extension UI bundles are served from — `{ext_ui_dir}/{ext}/{file}` (ui-federation
    /// scope). The bundle is **non-secret static code** (the session token is held by the shell, never
    /// the page; data access is gated at the host bridge), so it is served like any web asset. Seeded
    /// from `LB_EXT_UI_DIR` (default `extensions-ui` beside the cwd); tests point it at a fixture dir.
    pub ext_ui_dir: Arc<std::path::PathBuf>,
}

impl Gateway {
    /// Boot a gateway-role node with a freshly generated signing key and the real wall clock.
    /// Production entry point (the `node` binary / `serve`).
    pub async fn boot() -> Result<Self, String> {
        let node = Node::boot_as(lb_host::Role::Hub)
            .await
            .map_err(|e| e.to_string())?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Ok(Self::new(Arc::new(node), SigningKey::generate(), now))
    }

    /// Build a gateway around an existing node + an explicit signing key + clock. Lets the tests
    /// front a node with a known key (so they can forge/expire tokens) and a fixed clock.
    pub fn new(node: Arc<Node>, key: SigningKey, now: u64) -> Self {
        Self {
            node,
            key: Arc::new(key),
            now,
            trusted: Arc::new(TrustedKeys::new()),
            ext_ui_dir: Arc::new(
                std::env::var("LB_EXT_UI_DIR")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|_| std::path::PathBuf::from("extensions-ui")),
            ),
        }
    }

    /// Point the extension-UI serve dir at `dir` (builder-style) — tests serve a fixture bundle.
    pub fn with_ext_ui_dir(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.ext_ui_dir = Arc::new(dir.into());
        self
    }

    /// Seed the publisher allow-list the upload verifies against (the `POST /extensions` write path).
    /// Tests use this to install a known dev publisher; production leaves it empty until real
    /// publishers are wired. Returns `self` for builder-style construction.
    pub fn with_trusted(mut self, trusted: TrustedKeys) -> Self {
        self.trusted = Arc::new(trusted);
        self
    }
}
