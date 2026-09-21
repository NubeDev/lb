//! The gateway's BUILDERS — every `with_*` seam an embedder (or a test) uses to pin one piece of the
//! gateway's configuration after `new_live`/`new` has built it.
//!
//! Split out of `state.rs` because that file had reached the 400-line limit and the next seam would
//! have pushed it over. The split is along a real line rather than an arbitrary one: `state.rs` says
//! WHAT a gateway holds (the struct, its field docs — the wall each field defends) and how a bare one
//! is built; this file says how an embedder REPLACES one of those defaults. Adding a seam now grows
//! this file, and the reasoning for the field it fills stays beside the field.
//!
//! Every method is builder-style (`self` in, `Self` out) and every one is optional: a gateway that
//! calls none of them is the stock node.
//!
//! One responsibility: the configuration seams.

use std::sync::Arc;

use lb_registry::{Authenticity, TrustedKeys};

use super::state::{random_pepper, Gateway, PEPPER_ENV};

impl Gateway {
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

    /// Install the embedding product's identity for the `product` object on `GET /node` and
    /// `GET /health` (embedder-build-info scope). Builder-style; the boot seam passes
    /// `BootConfig::build_info`. Never called ⇒ the key is omitted from both bodies, which is the
    /// honest answer for the stock binary: lb is not embedded in anything.
    pub fn with_build_info(mut self, build_info: lb_discovery::BuildInfo) -> Self {
        self.build_info = Arc::new(Some(build_info));
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

    /// Name the workspace the pre-auth `GET /public/branding` answers for when a request names none
    /// (workspace-branding scope) — builder-style. The boot seam passes `BootConfig::public_brand_ws`;
    /// an empty string is treated as unset. Never called ⇒ the route is unchanged.
    pub fn with_public_brand_ws(mut self, ws: impl Into<String>) -> Self {
        let ws = ws.into();
        self.public_brand_ws = Arc::new(if ws.trim().is_empty() {
            None
        } else {
            Some(ws.trim().to_string())
        });
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
    pub(super) fn with_pepper_from_env(mut self) -> Self {
        match std::env::var(PEPPER_ENV) {
            Ok(p) if !p.is_empty() => self.pepper = Arc::from(p.into_bytes().into_boxed_slice()),
            _ => self.pepper = Arc::from(random_pepper().as_slice()),
        }
        self
    }
}
