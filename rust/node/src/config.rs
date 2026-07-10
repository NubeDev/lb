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
        }
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
