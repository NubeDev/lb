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

/// The env var a deployment sets to the API-key hash pepper (api-keys scope). The pepper keys the
/// HMAC over a key's secret field; it lives in env (a node secret), NEVER in the DB or a committed
/// constant. Unset in dev → a per-process random pepper (so API keys work locally but do not survive
/// a restart, like the dev-login). Tests inject a known pepper via [`Gateway::with_pepper`].
pub const PEPPER_ENV: &str = "LB_APIKEY_PEPPER";

/// The live node + the node's token-signing key, shared across handlers (`Arc` so axum can clone
/// it into each request). The key never leaves the node — the UI only ever holds the *issued*
/// token (scope: "Secrets").
#[derive(Clone)]
pub struct Gateway {
    pub node: Arc<Node>,
    /// The node's Ed25519 signing key — `login` mints with it, every route verifies with it.
    pub key: Arc<SigningKey>,
    /// A fixed "now" (unix seconds) injected by tests for determinism (testing §3 — no wall-clock):
    /// `Some(n)` pins the clock; `None` (production, via [`boot`]) reads the **live** wall clock per
    /// call through [`Gateway::now`]. The clock used to be a frozen field seeded once at boot — that
    /// froze every derived value (notably the flows run id `ts`) for the node's whole uptime, so
    /// every `flows.run` collided on one run id and raced the run-store
    /// (`debugging/flows/frozen-gw-now-collides-run-ids.md`). The test seam is preserved: a test
    /// constructs `Gateway::new(node, key, fixed_now)` and the clock stays pinned.
    pub fixed_now: Option<u64>,
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
    /// The API-key hash pepper (`HMAC-SHA256(pepper, secret_field)`), api-keys scope. A node secret
    /// from `LB_APIKEY_PEPPER` (never the DB, never committed); the dev default is a per-process
    /// random pepper so API keys work locally without a configured one. Held behind `Arc` so axum
    /// clones it cheaply per request.
    pub pepper: Arc<[u8]>,
}

impl Gateway {
    /// Boot a gateway-role node with a freshly generated signing key and the real wall clock.
    /// Production entry point (the `node` binary / `serve`).
    pub async fn boot() -> Result<Self, String> {
        let node = Node::boot_as(lb_host::Role::Hub)
            .await
            .map_err(|e| e.to_string())?;
        // Production reads the live wall clock per request (fixed_now = None) — never a value frozen
        // at boot. The wall-clock read lives in `Gateway::now`.
        Ok(Self::new_live(Arc::new(node), SigningKey::generate()).with_pepper_from_env())
    }

    /// The current unix-seconds clock: the injected fixed clock if a test pinned one, else a live
    /// wall-clock read. Use this everywhere a route needs "now" — it advances in production.
    pub fn now(&self) -> u64 {
        match self.fixed_now {
            Some(n) => n,
            None => std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        }
    }

    /// Build a gateway with a **live** wall clock (production). The token mint/verify clock advances
    /// per request via [`Gateway::now`].
    pub fn new_live(node: Arc<Node>, key: SigningKey) -> Self {
        Self::build(node, key, None)
    }

    /// Build a gateway around an existing node + an explicit signing key + a **fixed** clock. Lets
    /// the tests front a node with a known key (so they can forge/expire tokens) and a pinned clock.
    pub fn new(node: Arc<Node>, key: SigningKey, now: u64) -> Self {
        Self::build(node, key, Some(now))
    }

    fn build(node: Arc<Node>, key: SigningKey, fixed_now: Option<u64>) -> Self {
        // Install this key onto the NODE so there is ONE signing identity for the whole node: the
        // gateway's `login`/`authenticate` and the native tier's `LB_EXT_TOKEN` minter both read
        // `node.key()`. Without this a native sidecar's token would be minted with a throwaway key
        // the gateway can't verify (native-callback-transport scope). `Gateway::key` is kept as a
        // convenience clone of the same `Arc` for the routes that already read it.
        node.install_key(key.clone());
        Self {
            node,
            key: Arc::new(key),
            fixed_now,
            // Trust is environment, never the upload body: seed the publisher allow-list from
            // `LB_TRUSTED_PUBKEYS` (empty if unset → every upload 422s). Tests override via
            // `with_trusted`.
            trusted: Arc::new(crate::session::trusted_from_env()),
            ext_ui_dir: Arc::new(
                std::env::var("LB_EXT_UI_DIR")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|_| std::path::PathBuf::from("extensions-ui")),
            ),
            // Dev default: a per-process random pepper (no committed constant). Tests override.
            pepper: Arc::from(random_pepper().as_slice()),
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

    /// Set a known API-key pepper (tests). Production reads it from `LB_APIKEY_PEPPER` in [`boot`].
    pub fn with_pepper(mut self, pepper: impl Into<Arc<[u8]>>) -> Self {
        self.pepper = pepper.into();
        self
    }

    /// Read the API-key pepper from `LB_APIKEY_PEPPER`, or fall back to a per-process random pepper
    /// (dev — API keys work locally but don't survive a restart). Builder-style, used by [`boot`].
    fn with_pepper_from_env(mut self) -> Self {
        match std::env::var(PEPPER_ENV) {
            Ok(p) if !p.is_empty() => self.pepper = Arc::from(p.into_bytes().into_boxed_slice()),
            _ => self.pepper = Arc::from(random_pepper().as_slice()),
        }
        self
    }
}

/// 32 random bytes for the dev-default pepper (`rand`'s thread CSPRNG).
fn random_pepper() -> [u8; 32] {
    use rand::RngCore;
    let mut buf = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut buf);
    buf
}
