//! The gateway's shared state: the in-process node it fronts plus the **node signing key** it
//! mints and verifies session tokens with. The gateway IS a node (symmetric nodes, §3.1) — it
//! just also exposes an HTTP/SSE surface so a *browser* can reach it (README §6.13). It adds no
//! authority of its own; every route reads the caller's **bearer token**, verifies it with this
//! key (`session::authenticate`), and forwards to `lb_host::*` with the **verified principal** —
//! so the SAME capability check guards the browser as guards every other caller (capability-first,
//! §3.5), and the workspace comes from the *token*, never the request (the hard wall, §7).
//!
//! The demo principal is gone (collaboration scope, slice 1): `/auth/login` issues a real signed
//! token and every other route verifies it. The credential behind it is the person's ONE global
//! password (email-login scope); the legacy `POST /login {user, workspace}` was deleted in the
//! pre-production sweep, so `/auth/*` is the only human door. A real IdP plugs in behind the same
//! `GlobalCredentialCheck` seam later — `Non-goals` in the scope.

use std::sync::Arc;

use lb_auth::SigningKey;
use lb_host::Node;
use lb_registry::{Authenticity, TrustedKeys};

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
    /// Whether the publisher-signature half of the upload gate is **enforced** ([`Authenticity::Required`],
    /// the default and the only production posture) or **waived** for a development node
    /// (`LB_EXT_UNTRUSTED_KEY=allow`; see `session::trusted`).
    ///
    /// Waived, `POST /extensions` accepts an artifact whose `publisher_key_id` is not in [`Self::trusted`],
    /// and a registry pull accepts one from an unrecognised publisher. It does **not** waive the
    /// content-digest check — a corrupt or tampered artifact is still `422`. `Copy`, so no `Arc`.
    ///
    /// This value is *reported on `GET /health`* when waived. That is a deliberate trade the operator
    /// asked for: it makes a forgotten bench setting visible to whoever inherits the box without
    /// credentials or shell access, at the cost of also being visible to anyone who can reach the
    /// port. Do not enable this knob on a node exposed to an untrusted network.
    pub authenticity: Authenticity,
    /// The directory extension UI bundles are served from — `{ext_ui_dir}/{ext}/{file}` (ui-federation
    /// scope). The bundle is **non-secret static code** (the session token is held by the shell, never
    /// the page; data access is gated at the host bridge), so it is served like any web asset. Seeded
    /// from `LB_EXT_UI_DIR` (default `extensions-ui` beside the cwd); tests point it at a fixture dir.
    pub ext_ui_dir: Arc<std::path::PathBuf>,
    /// An optional static file tree served at the site root `/` as the router's fallback (static-root
    /// scope). `Some` ⇒ any request matching no API/ext-UI route is served from this dir via `ServeDir`
    /// (with `/`→`index.html`), making the node a self-contained web app host; `None` (the default) ⇒
    /// no fallback, unmatched paths 404 exactly as before. Non-secret static assets, same trust model
    /// as `ext_ui_dir`. Generic — the gateway never learns whose app it is (rule 10). Behind `Arc` so
    /// axum clones the state cheaply per request.
    pub static_root: Arc<Option<std::path::PathBuf>>,
    /// The browser-session (`/api/*`) seam, opt-in (browser-session scope). `Some(cfg)` ⇒ the gateway
    /// terminates a cookie session for a host that serves a shell: `/api/auth/*` mints/clears it and
    /// `ANY /api/{*rest}` resolves the sid to its stored bearer and dispatches internally to the same
    /// route a CLI would hit. `None` (the default) ⇒ today's bearer-only router, byte-for-byte: no
    /// `/api/*` route exists and no cookie is ever set (rubixd, rubix-ai, every existing node). Role is
    /// config, never a code branch (rule 2) — the gateway learns only "a shell is served and sessions
    /// are cookies", never whose. Behind `Arc` so axum clones the state cheaply per request.
    pub browser_session: Arc<Option<crate::browser_session::BrowserSessionConfig>>,
    /// The API-key hash pepper (`HMAC-SHA256(pepper, secret_field)`), api-keys scope. A node secret
    /// from `LB_APIKEY_PEPPER` (never the DB, never committed); the dev default is a per-process
    /// random pepper so API keys work locally without a configured one. Held behind `Arc` so axum
    /// clones it cheaply per request.
    pub pepper: Arc<[u8]>,
    /// The credential check `/auth/login` runs before minting a token (email-login scope) — the ONE
    /// credential seam. Verifies the person's ONE global password (`identity_credential`) after the
    /// email→sub lookup. Selected by `LB_DEV_LOGIN` in
    /// production ([`boot`]): set → `GlobalDevTrustAny` (dev/CI, password-less); unset →
    /// `GlobalPasswordHash` (argon2). Tests override via [`Gateway::with_global_credential_check`]; the
    /// `new`/`new_live` seams default to `GlobalDevTrustAny`. Behind `Arc<dyn>` for cheap axum clones.
    pub global_credential_check: Arc<dyn crate::session::GlobalCredentialCheck>,
    /// The unified-event-stream hub (unified-event-stream scope): the process-wide, ephemeral registry
    /// of the browser's one multiplexed SSE connection per session and its live subject subscriptions.
    /// No durable state — a dropped connection drops its subscriptions. Shared (`Clone`d cheaply) across
    /// handlers so the `GET /events/stream` body and the `POST /events/{sid}/*` control verbs address
    /// the same connections.
    pub events: crate::session::events::EventHub,
    /// The route-scoped body-size ceiling (bytes) for the `POST /extensions` artifact upload
    /// (extension-upload-limit fix). A native-tier artifact packs a host-target binary (megabytes)
    /// into the signed Artifact's JSON-encoded `wasm` field, so a real sidecar upload runs to
    /// hundreds of MiB — far past axum's 2 MiB `DefaultBodyLimit` default, which used to 413 before the
    /// body was read. `router` reads this to size the `DefaultBodyLimit` layer on THAT ONE route
    /// (never a global bump — rule 10) and the handler reports a descriptive over-limit error. Bounded
    /// (not unlimited) so a runaway upload can't OOM the node. Sourced from `BootConfig`
    /// (`max_extension_upload_bytes`, default 384 MiB) at the boot seam; the test/`new_live` seams
    /// default via [`Gateway::DEFAULT_MAX_EXTENSION_UPLOAD_BYTES`].
    pub max_extension_upload_bytes: u64,
    /// The in-memory health cell `GET /health` reads (issue #72). One `AtomicBool` per subsystem
    /// the fleet health contract names (`store`, `gateway`); load-only reads so the probe never
    /// blocks on a dependency. Both default to serving — see `routes/health` for why that is the
    /// honest answer today (the store handle is alive once `Node::boot` opened it; the gateway is
    /// handling the request). The per-subsystem setters are the seam a future in-process monitor
    /// (a store-down detector, a drain-on-shutdown handoff) flips without the route shape changing;
    /// no caller flips them yet. Shared behind `Arc` so axum clones the state cheaply per request.
    pub health: crate::routes::SharedHealthGate,

    /// Who this node is, for the unauthenticated `GET /node` probe (node-identity scope). `None`
    /// (the default, and every existing test/`new_live` seam) ⇒ the route `404`s: the node has only
    /// the throwaway per-boot id and has no durable identity to publish. Installed by the boot seam
    /// from `BootConfig::identity` via [`Gateway::with_identity`].
    ///
    /// Carries nothing beyond what a discoverable node already broadcasts over mDNS in cleartext —
    /// see `routes::node_identity` for the wall argument. Behind `Arc` so axum clones the state
    /// cheaply per request.
    pub identity: Arc<Option<lb_discovery::NodeIdentity>>,

    /// The port `GET /node` reports as this node's dialable gateway endpoint. `None` ⇒ the route
    /// reports `0`; the boot seam pins the REAL bound port (which may differ from the requested one
    /// when binding `:0`), so what is published is what a caller can actually dial.
    pub bound_port: Option<u16>,

    /// The embedder-registered **upload sinks** (node-update scope §Seam 2), keyed by an opaque
    /// name. Empty (the default, and every existing node) ⇒ the `/uploads/*` routes are NOT MOUNTED
    /// at all — byte-for-byte today's behaviour, and an unmounted route cannot be a surface.
    ///
    /// The gateway never learns whose bytes it moves (rule 10): it owns framing, resumption, bounds
    /// and the wall; the sink owns the backend, the digest verification and the durable state.
    /// Behind `Arc` so axum clones the state cheaply per request.
    pub upload_sinks: Arc<Vec<(String, Arc<dyn lb_host::UploadSink>)>>,

    /// Candidate addresses `GET /node` publishes. Empty is normal and honest — a wildcard bind with
    /// no embedder-supplied enumeration has no specific address worth naming, and publishing
    /// `0.0.0.0` would hand a client something unroutable. See `routes::node_identity` for why this
    /// is a list rather than one address.
    pub bound_addresses: Arc<[std::net::IpAddr]>,
}

impl Gateway {
    /// The default `POST /extensions` upload ceiling when no explicit limit is set (test/`new_live`
    /// seams): 384 MiB — sized to the largest real native sidecar artifact seen (the ems modbus
    /// bundle ~317 MiB) with headroom, and bounded so a runaway upload can't OOM the node. The
    /// production boot path overrides this from `BootConfig::max_extension_upload_bytes`
    /// (`lb_node::DEFAULT_MAX_EXTENSION_UPLOAD_BYTES`, kept numerically equal to this).
    pub const DEFAULT_MAX_EXTENSION_UPLOAD_BYTES: u64 = 384 * 1024 * 1024;

    /// Boot a gateway-role node with the resolved signing key and the real wall clock.
    /// Production entry point (the `node` binary / `serve`).
    pub async fn boot() -> Result<Self, String> {
        let node = Node::boot_as(lb_host::Role::Hub)
            .await
            .map_err(|e| e.to_string())?;
        // The signing key is PERSISTED beside a durable store (`LB_STORE_PATH`) so a browser session
        // survives a node restart — a fresh-per-boot key silently 401'd every rehydrated token and
        // read paths fell back to empty (e.g. the agent catalog showed "No agent definitions
        // available" over a store that still held them). See `crate::signing_key`. An in-memory node
        // still gets a fresh ephemeral key (nothing durable to pair it with).
        // Production reads the live wall clock per request (fixed_now = None) — never a value frozen
        // at boot. The wall-clock read lives in `Gateway::now`.
        // Select the credential check from the environment: `LB_DEV_LOGIN` set → `GlobalDevTrustAny`
        // (dev/CI, password-less); unset → `GlobalPasswordHash` (a real argon2 credential is required —
        // the release default hard-refuses a password-less login). Only the env-driven production
        // `boot` path applies this gate; the `new`/`new_live` test seams stay password-less.
        Ok(
            Self::new_live(Arc::new(node), crate::signing_key::resolve())
                .with_pepper_from_env()
                .with_global_credential_check(crate::session::global_credential_check_from_env()),
        )
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
            // Fail closed: the publisher-signature check is enforced unless `LB_EXT_UNTRUSTED_KEY`
            // holds the exact disable token. `authenticity_from_env` warns on stderr when it does.
            // Tests and embedders override via `with_authenticity` / `BootConfig`.
            authenticity: crate::session::authenticity_from_env(),
            ext_ui_dir: Arc::new(
                std::env::var("LB_EXT_UI_DIR")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|_| std::path::PathBuf::from("extensions-ui")),
            ),
            // No static-root fallback by default (unmatched paths 404, unchanged). An embedder pins one
            // via `with_static_root`; the boot seam fills it from `BootConfig::static_root`.
            static_root: Arc::new(None),
            // No browser-session seam by default: the router stays bearer-only exactly as today. An
            // embedder opts in via `with_browser_session`; the boot seam fills it from
            // `BootConfig::browser_session` (browser-session scope).
            browser_session: Arc::new(None),
            // Dev default: a per-process random pepper (no committed constant). Tests override.
            pepper: Arc::from(random_pepper().as_slice()),
            // The credential check `/auth/login` runs before minting (email-login scope).
            // Password-less on the `new`/`new_live` test seams, env-driven in production `boot`
            // (which hard-refuses a password-less login in release). A test uses
            // `with_global_credential_check` to exercise the real
            // `GlobalPasswordHash` (`401` on bad/absent global secret) path against a seeded credential.
            global_credential_check: Arc::new(crate::session::GlobalDevTrustAny),
            events: crate::session::events::EventHub::new(),
            // The `POST /extensions` upload ceiling — the safe default until the boot path pins the
            // configured value via `with_max_extension_upload_bytes`.
            max_extension_upload_bytes: Self::DEFAULT_MAX_EXTENSION_UPLOAD_BYTES,
            // The `GET /health` gate — both subsystems serving. See `routes::health` for why this is
            // the honest default (the store handle is alive once the node booted it; a future monitor
            // flips a subsystem via the `HealthGate` setters).
            health: Arc::new(crate::routes::HealthGate::new()),
            // No durable identity on the test/`new_live` seams — `GET /node` 404s until the boot
            // path installs one from `BootConfig::identity` (node-identity scope).
            identity: Arc::new(None),
            bound_port: None,
            bound_addresses: Arc::from(Vec::new()),
            // No upload sinks by default: the `/uploads/*` routes are not mounted (node-update
            // scope §Seam 2). The boot seam fills this from `BootConfig::upload_sinks`.
            upload_sinks: Arc::new(Vec::new()),
        }
    }

    /// Install the node's durable identity + its dialable endpoint for `GET /node` (node-identity
    /// scope). Builder-style; the boot seam passes `BootConfig::identity` and the REAL bound
    /// address. Never called ⇒ the route `404`s, which is the honest answer for a node whose only
    /// id is the per-boot random one.
    pub fn with_identity(
        mut self,
        identity: lb_discovery::NodeIdentity,
        port: u16,
        addresses: Vec<std::net::IpAddr>,
    ) -> Self {
        self.identity = Arc::new(Some(identity));
        self.bound_port = Some(port);
        self.bound_addresses = Arc::from(addresses);
        self
    }

    /// Register the embedder's upload sinks (node-update scope §Seam 2) — builder-style; the boot
    /// seam passes `BootConfig::upload_sinks`, and a test registers its own real sink. Never called
    /// (or called with an empty vec) ⇒ the `/uploads/*` routes are not mounted, unchanged.
    pub fn with_upload_sinks(mut self, sinks: Vec<(String, Arc<dyn lb_host::UploadSink>)>) -> Self {
        self.upload_sinks = Arc::new(sinks);
        self
    }

    /// Pin the `POST /extensions` upload ceiling (bytes) the `router` sizes its route-scoped
    /// `DefaultBodyLimit` from (extension-upload-limit fix). Builder-style; the boot seam passes
    /// `BootConfig::max_extension_upload_bytes`, tests pin a small value to exercise the reject path.
    pub fn with_max_extension_upload_bytes(mut self, bytes: u64) -> Self {
        self.max_extension_upload_bytes = bytes;
        self
    }

    /// Install the GLOBAL credential check `/auth/login` runs before minting (email-login scope).
    /// Production `boot` selects it from `LB_DEV_LOGIN`; a test uses this to exercise the real
    /// `GlobalPasswordHash` (`401` on bad/absent global secret) against a seeded credential.
    pub fn with_global_credential_check(
        mut self,
        check: Arc<dyn crate::session::GlobalCredentialCheck>,
    ) -> Self {
        self.global_credential_check = check;
        self
    }

    /// Point the extension-UI serve dir at `dir` (builder-style) — tests serve a fixture bundle.
    pub fn with_ext_ui_dir(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.ext_ui_dir = Arc::new(dir.into());
        self
    }

    /// Serve a static file tree at the site root `/` as the router's fallback (static-root scope) —
    /// an embedder points this at a self-contained web app; tests point it at a fixture dir. Builder-
    /// style. Unset (the default) leaves the router with no fallback (unmatched paths 404).
    pub fn with_static_root(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.static_root = Arc::new(Some(dir.into()));
        self
    }

    /// Terminate a cookie-backed browser session at `/api/*` (browser-session scope) — builder-style.
    /// An embedder whose shell lb serves (`with_static_root`) opts in here; unset (the default) leaves
    /// the router bearer-only, with no `/api/*` routes and no cookies anywhere.
    pub fn with_browser_session(
        mut self,
        cfg: crate::browser_session::BrowserSessionConfig,
    ) -> Self {
        self.browser_session = Arc::new(Some(cfg));
        self
    }

    /// Seed the publisher allow-list the upload verifies against (the `POST /extensions` write path).
    /// Tests use this to install a known dev publisher; production leaves it empty until real
    /// publishers are wired. Returns `self` for builder-style construction.
    pub fn with_trusted(mut self, trusted: TrustedKeys) -> Self {
        self.trusted = Arc::new(trusted);
        self
    }

    /// Pin whether the publisher-signature check is enforced or waived (the development escape
    /// hatch). Production leaves this at [`Authenticity::Required`]; the boot seam fills it from
    /// `LB_EXT_UNTRUSTED_KEY` / `BootConfig::authenticity`, and tests use it to exercise both
    /// postures without touching process-wide env. Returns `self` for builder-style construction.
    pub fn with_authenticity(mut self, authenticity: Authenticity) -> Self {
        self.authenticity = authenticity;
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
