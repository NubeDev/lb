//! [`BootConfig`] — the struct an embedder fills to boot a node through [`boot_full`](crate::boot_full).
//!
//! This is the **supported embed API's** config surface (node-roles / embed scope). It carries
//! everything the `node` binary today reads from `LB_*` env at boot; a third-party embedder
//! (`NubeIO/rubix-ai`, git-dep on `NubeDev/lb`) constructs it directly and **passes no env** — the
//! doctrine that env is a *binary* concern, read only at the binary boundary, never below the boot
//! seam. The ONE place `LB_*` boot vars are read is [`BootConfig::from_env`], which reproduces today's
//! binary behaviour exactly; the `node` binary's `main.rs` calls it and nothing else does.
//!
//! The struct is `#[non_exhaustive]` + `Default` with all-`pub` fields, so a downstream embedder
//! constructs a partial config by mutating `default()` — `let mut c = BootConfig::default(); c.store_path
//! = Some(..);` — and new boot inputs land as additive fields without breaking that call (the
//! API-commitment mitigation the scope names). NOTE: `#[non_exhaustive]` deliberately forbids a
//! cross-crate struct *literal* (`BootConfig { .. }`), which is exactly what makes the additive
//! guarantee hold; the `default()`-then-mutate form is the supported construction path.

use std::net::SocketAddr;

use lb_auth::SigningKey;

/// Where the gateway (if any) binds / how the embedder takes the served HTTP surface.
///
/// `Off` is the headless posture (store + auth + MCP in-process, no HTTP) — the default an embedder
/// wants. `Addr(..)` mirrors today's `LB_GATEWAY_ADDR`: the ritual builds the gateway, installs its
/// key, mounts the native roles, and serves on that address. (A `Listener` variant — hand the gateway
/// back for the embedder's own `serve_listener` — is a documented follow-up; `serve_listener` exists
/// on the gateway but is not yet threaded through the config, so it is intentionally omitted here
/// rather than stubbed.)
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub enum GatewayMode {
    /// No HTTP gateway: a headless node exposing host verbs in-process (the embed default).
    #[default]
    Off,
    /// Serve the SSE/HTTP gateway on this address (today's `LB_GATEWAY_ADDR` posture).
    Addr(SocketAddr),
}

/// Which credential check `POST /login` runs before minting a token (embedder-credential-mode
/// scope — the embed-seam completion of login-hardening). The gateway already ships two checks
/// behind the `CredentialCheck` trait; this selects which one a `boot_full` node installs, through
/// the existing `Gateway::with_credential_check` seam. Selecting it here (a `BootConfig` field) is
/// what lets an embedded node run **real** password login — before this, `builder.rs` hardwired
/// `DevTrustAny` and an embedded `POST /login` accepted any password.
///
/// `Default` is [`DevTrustAny`](CredentialMode::DevTrustAny) — the back-compat embed default, so
/// every existing embedder and `boot_full`-based test keeps today's password-less login until it
/// opts in. `from_env()` differs on purpose: it mirrors the standalone binary's `LB_DEV_LOGIN`
/// rule (unset ⇒ `PasswordHash`), reproducing the `node` binary exactly.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum CredentialMode {
    /// Password-less: trust any `(user, workspace)` with no secret (today's `boot_full` default,
    /// and the standalone binary's `LB_DEV_LOGIN=1` mode). Dev/CI only — the token is still
    /// role-scoped, so a dev member is not an admin.
    #[default]
    DevTrustAny,
    /// The real check: argon2 against the stored per-`(ws, user)` credential. A wrong/absent secret
    /// `401`s with no token (the standalone binary's `LB_DEV_LOGIN`-unset mode).
    PasswordHash,
}

/// The in-house agent model config — the `ModelEndpoint` shape (provider / model / api-key-env NAME /
/// base-url). Names only: `api_key_env` is the NAME of an env var holding the key, never the key value.
/// `None` provider ⇒ the honest [`UnconfiguredModel`](lb_host::UnconfiguredModel) is kept.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct AgentModelConfig {
    /// The provider id (`zaicoding` / `openai` / `openai-compat`); empty/absent ⇒ unconfigured.
    pub provider: Option<String>,
    /// The model id.
    pub model: String,
    /// The NAME of the env var holding the API key (resolved at the binary boundary, never logged).
    pub api_key_env: String,
    /// An optional base URL override for the OpenAI-compatible transport.
    pub base_url: Option<String>,
}

/// The **outbox delivery providers** an embedder injects at boot (release scope, gap 1 — the
/// provider-injection seam). Each is the one sanctioned external behind its host trait; `None`
/// falls back to the logging no-op provider so boot never crashes and the relay still drains
/// (the send is logged, not performed). The `node` binary leaves both `None` today; a product
/// host fills them with its real SMTP/WebPush/FCM adapters.
#[derive(Clone, Default)]
#[non_exhaustive]
pub struct OutboxProviders {
    /// The email delivery provider (`EmailTarget`'s external). `None` ⇒ logging no-op.
    pub email: Option<std::sync::Arc<dyn lb_host::EmailProvider>>,
    /// The push delivery provider (`PushTarget`'s external). `None` ⇒ logging no-op.
    pub push: Option<std::sync::Arc<dyn lb_host::PushProvider>>,
}

/// Everything the boot ritual needs. Filled at the binary boundary (env today, via [`from_env`]) or by
/// an embedder (mutating [`default()`](Default::default)). No library code below the boot seam reads
/// this from env.
#[derive(Clone)]
#[non_exhaustive]
pub struct BootConfig {
    /// The durable store path (today's `LB_STORE_PATH`). `Some(non-empty)` ⇒ an on-disk `Store::open`
    /// (durable across restart); `None`/empty ⇒ an ephemeral `mem://` store (dev/test/embed default).
    pub store_path: Option<String>,

    /// The node's token-signing key (today's `LB_SIGNING_KEY`, 64-hex seed → `SigningKey`). Custody is
    /// the embedder's: filled here at the binary boundary, never logged. Defaults to a fresh per-boot
    /// key (matching `make dev`/test behaviour when `LB_SIGNING_KEY` is unset).
    pub signing_key: SigningKey,

    /// The boot workspace (today's `LB_WORKSPACE`, default `acme`). The dev-identity seed, extension
    /// re-load, reactors, and default-skill grants all scope to this workspace.
    pub workspace: String,

    /// The dev identity to seed as a `workspace-admin` member of `workspace` (today's `LB_SEED_USER`,
    /// default `user:ada`). `None` skips the seed entirely (an embedder that provisions its own
    /// identities). Idempotent when present.
    pub seed_user: Option<String>,

    /// Gateway posture (today's `LB_GATEWAY_ADDR` presence). `Off` = headless; `Addr` = serve HTTP.
    pub gateway: GatewayMode,

    /// Run the background reactor loops (flow / agent / approval / insight-digest scans). `true`
    /// reproduces today's binary. An embedder wanting store+auth+MCP only sets `false` — no scans run.
    pub reactors: bool,

    /// Load + call the `hello` demo extension at boot (today's unconditional S1 bring-up). `true`
    /// reproduces the binary; an embedder wants this `false` (no demo extension, no `hello.echo` call).
    pub hello_demo: bool,

    /// The default core-skill id set to grant the boot workspace (today's `LB_DEFAULT_CORE_SKILLS`,
    /// comma-separated; `None` ⇒ the compiled-in read-only defaults; `Some("")` ⇒ none).
    pub default_core_skills: Option<String>,

    /// The telemetry sink selection (today's `LB_TELEMETRY_SINK`). Applied right after boot.
    pub telemetry: lb_telemetry::SinkConfig,

    /// The in-house agent model config (today's `LB_AGENT_MODEL_*`). The `default` runtime binds this.
    pub agent_model: AgentModelConfig,

    /// The served agent actor's capability ceiling (today's `LB_AGENT_CAPS`, comma-separated). `None`
    /// ⇒ the default platform-tool surface. Always intersected with the caller at the wall.
    pub agent_caps: Option<Vec<String>>,

    /// Where the gateway serves installed extension **UI bundles** from — `{ext_ui_dir}/{ext}/{file}`,
    /// reachable at `GET /extensions/{ext}/ui/{file}` (ui-federation scope). Mirrors `store_path`'s
    /// posture: `Some` ⇒ the gateway is built with `Gateway::with_ext_ui_dir(dir)`, pinning the serve
    /// dir to an embedder-chosen (typically absolute) path; `None` ⇒ today's unchanged behaviour — the
    /// gateway keeps its own `LB_EXT_UI_DIR`/`"extensions-ui"` default read at the binary boundary. Like
    /// every other field, no library code below the seam reads this from env — an embedder fills it.
    pub ext_ui_dir: Option<String>,

    /// The outbox delivery providers (email/push) the relay reactor delivers through (release
    /// scope, gap 1). Additive: `Default` (both `None`) keeps prior behaviour safe — the relay
    /// spawns with logging no-op providers, so effects drain and boot never crashes for lack of
    /// delivery config. An embedder fills these with real adapters.
    pub outbox_providers: OutboxProviders,

    /// Which credential check `POST /login` runs before minting (embedder-credential-mode scope).
    /// Additive: `Default` is [`CredentialMode::DevTrustAny`] — today's `boot_full` password-less
    /// behaviour, so no existing embedder breaks. An embedder sets [`CredentialMode::PasswordHash`]
    /// to enforce real passwords (argon2 against the stored credential); `from_env()` derives it
    /// from `LB_DEV_LOGIN` to reproduce the standalone binary. Applied through
    /// `Gateway::with_credential_check` in the builder's `GatewayMode::Addr` arm; irrelevant when
    /// the gateway is `Off` (no login route).
    pub credential_mode: CredentialMode,

    /// An optional password to seed for the dev [`seed_user`](Self::seed_user) at boot, so a node
    /// running [`CredentialMode::PasswordHash`] has a first admin who can actually log in (the
    /// bootstrap-paradox fix: `identity.set_credential` needs an admin token, but no admin can
    /// authenticate until a credential exists). `Some(non-empty)` ⇒ the boot seed argon2-hashes it
    /// into the seed user's credential record (idempotent, alongside the membership seed). `None`
    /// (the default) seeds no credential — correct for `DevTrustAny` nodes (password-less) and for
    /// an embedder that provisions credentials its own way. Secret-class: filled at the binary
    /// boundary, hashed before write, never logged (mirrors `signing_key`'s custody).
    pub seed_credential: Option<String>,
}

impl Default for BootConfig {
    /// The embed-friendly default: `mem://` store, a fresh signing key, workspace `acme`, dev-identity
    /// seed on, gateway OFF, reactors ON, **hello demo OFF** (an embedder does not want it), default
    /// core skills, no telemetry, an unconfigured agent model. Note `hello_demo` is `false` here but
    /// `from_env()` sets it `true` to reproduce today's binary exactly.
    fn default() -> Self {
        Self {
            store_path: None,
            signing_key: SigningKey::generate(),
            workspace: "acme".into(),
            seed_user: Some("user:ada".into()),
            gateway: GatewayMode::Off,
            reactors: true,
            hello_demo: false,
            default_core_skills: None,
            telemetry: lb_telemetry::SinkConfig::Off,
            agent_model: AgentModelConfig::default(),
            agent_caps: None,
            // `None` ⇒ the gateway keeps its own `LB_EXT_UI_DIR`/"extensions-ui" default (the standalone
            // binary is untouched); an embedder sets an absolute path to relocate the ext-UI serve dir.
            ext_ui_dir: None,
            outbox_providers: OutboxProviders::default(),
            // Back-compat embed default: password-less. An embedder opts into `PasswordHash`
            // explicitly; `from_env()` (below) derives it from `LB_DEV_LOGIN` for the binary.
            credential_mode: CredentialMode::DevTrustAny,
            // No seed password by default — a `DevTrustAny` node needs none, and an embedder fills
            // this only when it boots `PasswordHash` and wants the dev admin to be able to log in.
            seed_credential: None,
        }
    }
}

impl BootConfig {
    /// Read the boot config from `LB_*` env, reproducing today's `node` binary behaviour EXACTLY. This
    /// is the ONLY place boot env vars are read; only binaries call it. Embedders construct [`BootConfig`]
    /// directly and never touch env.
    pub fn from_env() -> Self {
        BootConfig {
            store_path: std::env::var("LB_STORE_PATH")
                .ok()
                .filter(|p| !p.is_empty()),
            signing_key: gateway_signing_key(),
            workspace: std::env::var("LB_WORKSPACE").unwrap_or_else(|_| "acme".into()),
            seed_user: Some(std::env::var("LB_SEED_USER").unwrap_or_else(|_| "user:ada".into())),
            gateway: gateway_mode_from_env(),
            reactors: true,
            // The binary loads + calls the hello demo unconditionally today.
            hello_demo: true,
            default_core_skills: std::env::var("LB_DEFAULT_CORE_SKILLS").ok(),
            telemetry: lb_telemetry::SinkConfig::from_env(),
            agent_model: agent_model_from_env(),
            agent_caps: agent_caps_from_env(),
            // Left `None` on the binary path ON PURPOSE: the gateway reads `LB_EXT_UI_DIR` itself in
            // `Gateway::build`, so the standalone `node` binary's ext-UI serve dir is unchanged. Only an
            // embedder (filling the struct directly) uses this field to relocate the dir off env.
            ext_ui_dir: None,
            // The binary configures no real delivery providers today — the relay drains through
            // the logging no-ops. Real adapters come from an embedder filling the struct.
            outbox_providers: OutboxProviders::default(),
            // Mirror the standalone gateway's `credential_check_from_env`: `LB_DEV_LOGIN` set/
            // non-empty ⇒ `DevTrustAny` (password-less dev/CI), unset ⇒ `PasswordHash` (the real
            // argon2 check). This reproduces the `node` binary exactly — the binary's default is
            // password-enforcing, unlike the embed `Default` which stays `DevTrustAny` for
            // back-compat.
            credential_mode: credential_mode_from_env(),
            // Optional dev-admin seed password (`LB_SEED_PASSWORD`) — so a `PasswordHash` binary
            // has a first admin who can log in. Absent ⇒ no credential seeded (correct for a
            // `DevTrustAny` binary). Secret-class: read here, hashed at seed time, never logged.
            seed_credential: std::env::var("LB_SEED_PASSWORD").ok().filter(|s| !s.is_empty()),
        }
    }
}

/// Map `LB_DEV_LOGIN` to a [`CredentialMode`], matching the standalone gateway's
/// `credential_check_from_env` rule: set/non-empty ⇒ `DevTrustAny`, unset/empty ⇒ `PasswordHash`.
/// Only `from_env` (the binary boundary) reads this env — the field carries the choice below the
/// boot seam so no library code re-reads env.
fn credential_mode_from_env() -> CredentialMode {
    match std::env::var("LB_DEV_LOGIN") {
        Ok(v) if !v.trim().is_empty() => CredentialMode::DevTrustAny,
        _ => CredentialMode::PasswordHash,
    }
}

/// Parse `LB_GATEWAY_ADDR` into a [`GatewayMode`]. Unset ⇒ `Off` (solo/headless). A malformed address
/// is fatal at the binary boundary today (`main` returned an error) — we surface it the same way by
/// letting the parse fail loudly at boot. To keep `from_env` infallible (its callers expect a value),
/// a malformed addr falls back to `Off` with a warning, matching the "don't panic in boot config"
/// posture; the address is re-validated in the builder.
fn gateway_mode_from_env() -> GatewayMode {
    match std::env::var("LB_GATEWAY_ADDR") {
        Ok(addr) if !addr.is_empty() => match addr.parse::<SocketAddr>() {
            Ok(a) => GatewayMode::Addr(a),
            Err(e) => {
                eprintln!("bad LB_GATEWAY_ADDR '{addr}': {e} — starting headless (no gateway)");
                GatewayMode::Off
            }
        },
        _ => GatewayMode::Off,
    }
}

/// The gateway's signing key from `LB_SIGNING_KEY` (64 hex = 32-byte Ed25519 seed) when set, else a
/// fresh one per boot. Moved verbatim from the old `main.rs` (the binary-boundary key custody, §3.1):
/// a deployed node wants a key that survives restart; a fresh key matches `make dev`/test.
fn gateway_signing_key() -> SigningKey {
    let Ok(hex_seed) = std::env::var("LB_SIGNING_KEY") else {
        return SigningKey::generate();
    };
    let hex_seed = hex_seed.trim();
    if hex_seed.len() != 64 {
        eprintln!(
            "LB_SIGNING_KEY: expected 64 hex chars (32-byte seed), got {} — generating a fresh key",
            hex_seed.len()
        );
        return SigningKey::generate();
    }
    let mut seed = [0u8; 32];
    for (i, byte) in seed.iter_mut().enumerate() {
        match u8::from_str_radix(&hex_seed[i * 2..i * 2 + 2], 16) {
            Ok(b) => *byte = b,
            Err(_) => {
                eprintln!("LB_SIGNING_KEY: not valid hex — generating a fresh key");
                return SigningKey::generate();
            }
        }
    }
    SigningKey::from_seed(&seed)
}

/// Read the in-house model config from `LB_AGENT_MODEL_*` (provider / model / api-key-env NAME /
/// base-url). An absent/empty provider ⇒ `provider: None` (the honest unconfigured state).
fn agent_model_from_env() -> AgentModelConfig {
    let provider = std::env::var("LB_AGENT_MODEL_PROVIDER")
        .ok()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());
    AgentModelConfig {
        provider,
        model: std::env::var("LB_AGENT_MODEL_MODEL").unwrap_or_default(),
        api_key_env: std::env::var("LB_AGENT_MODEL_API_KEY_ENV").unwrap_or_default(),
        base_url: std::env::var("LB_AGENT_MODEL_BASE_URL").ok(),
    }
}

/// Read `LB_AGENT_CAPS` (comma-separated). `None` ⇒ the served agent uses its default cap ceiling.
fn agent_caps_from_env() -> Option<Vec<String>> {
    let raw = std::env::var("LB_AGENT_CAPS").ok()?;
    let caps: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    (!caps.is_empty()).then_some(caps)
}
