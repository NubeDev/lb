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
use lb_role_gateway::Authenticity;

use crate::store_env::{store_budget_bytes_from_env, store_open_unguarded_from_env};

/// The default `POST /extensions` upload ceiling (extension-upload-limit fix): 384 MiB. Sized to the
/// largest real native sidecar artifact observed (the ems modbus bundle ~317 MiB — a 6.2 MB release
/// binary is ~50 MB once JSON-encoded, and a 317 MB embedder bundle is the current high-water mark)
/// with headroom, and deliberately bounded (never unlimited) so a runaway upload can't OOM the node.
pub const DEFAULT_MAX_EXTENSION_UPLOAD_BYTES: u64 = 384 * 1024 * 1024;

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

/// Which credential check `POST /auth/login` runs before minting a token (embedder-credential-mode
/// scope — the embed-seam completion of login-hardening). The gateway ships two checks behind the
/// `GlobalCredentialCheck` trait; this selects which one a `boot_full` node installs, through the
/// `Gateway::with_global_credential_check` seam. Selecting it here (a `BootConfig` field) is what
/// lets an embedded node run **real** password login — before this, `builder.rs` hardwired the
/// password-less check and an embedded login accepted any password.
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

/// The **outbox delivery providers + targets** an embedder injects at boot (release scope, gap 1).
/// A `None` provider falls back to the logging no-op, so boot never crashes and the relay still
/// drains (the send is logged, not performed); the `node` binary leaves both `None`.
#[derive(Clone, Default)]
#[non_exhaustive]
pub struct OutboxProviders {
    /// The email delivery provider (`EmailTarget`'s external). `None` ⇒ logging no-op.
    pub email: Option<std::sync::Arc<dyn lb_host::EmailProvider>>,
    /// The push delivery provider (`PushTarget`'s external). `None` ⇒ logging no-op.
    pub push: Option<std::sync::Arc<dyn lb_host::PushProvider>>,
    /// Embedder-registered targets keyed by the opaque `effect.target`, folded in AFTER the
    /// built-ins so a host may add one or replace one. Names nothing (rule 10).
    pub targets: Vec<(String, std::sync::Arc<dyn lb_host::DynTarget>)>,
}

/// The datasource **discovery profile** knobs (datasource-profile scope). Plain data, always
/// compiled (like `CacheConfig`) so the field's type exists whether or not the feature is on — an
/// embedder's config code must not need `#[cfg]`.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct ProfileConfig {
    /// The runtime switch. `false` ⇒ the reactor is not spawned; the verbs still work when called
    /// explicitly. This separation is deliberate: an operator may want on-demand profiling without
    /// a clock spending work on their database.
    pub enabled: bool,
    /// A profile older than this is re-enqueued by the reactor. Default 24 h — the scope's global
    /// setting; a per-datasource override is a later, additive field on the datasource record.
    pub refresh_after_secs: u64,
    /// Tables per pass. Clamped down by the sidecar's ceiling, never up.
    pub max_tables: u64,
    /// Distinct values retained per text column. Clamped down by the sidecar's ceiling, never up.
    pub max_values: u64,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            refresh_after_secs: 86_400,
            max_tables: 25,
            max_values: 60,
        }
    }
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

    /// An optional static file tree the gateway serves at the site root `/` as a fallback route
    /// (static-root scope). `Some(dir)` ⇒ any request that matches no API/ext-UI route is served from
    /// `{static_root}/{path}`, with `/` resolving to that dir's `index.html` — turning the node into a
    /// self-contained web app host (an embedder bakes its shell/SPA there; the ARM/Pi single-binary
    /// build points this at its embedded shell FS). `None` (the default) ⇒ today's unchanged behaviour:
    /// no fallback, unmatched paths 404. Generic and embedder-agnostic — no library code below the seam
    /// reads this from env, and the gateway never learns whose shell it is (rule 10). Mirrors
    /// [`ext_ui_dir`](Self::ext_ui_dir)'s posture exactly.
    pub static_root: Option<String>,

    /// Terminate a cookie-backed **browser session** at `/api/*` (browser-session scope). `Some(cfg)`
    /// ⇒ the gateway mints an `HttpOnly` session cookie at `/api/auth/login`, keeps the JWT
    /// server-side in the store, and resolves `ANY /api/{*rest}` to the bearer before dispatching
    /// internally to the same route a CLI would hit — so a host whose shell lb serves
    /// ([`static_root`](Self::static_root)) gets a working browser login without hand-rolling a
    /// security boundary, and the fat (~4–9KB, over the cookie limit) token never reaches JS.
    /// `None` (the default) ⇒ today's unchanged behaviour: bearer-only, no `/api/*` route, no cookie
    /// anywhere — rubixd, rubix-ai, and every existing embedder are untouched. Additive and
    /// embedder-agnostic: no library code below the seam reads this from env, and the gateway never
    /// learns whose shell it is (rule 10). Typically set alongside `static_root`.
    pub browser_session: Option<lb_role_gateway::BrowserSessionConfig>,

    /// The outbox delivery providers (email/push) the relay reactor delivers through (release
    /// scope, gap 1). Additive: `Default` (both `None`) keeps prior behaviour safe — the relay
    /// spawns with logging no-op providers, so effects drain and boot never crashes for lack of
    /// delivery config. An embedder fills these with real adapters.
    pub outbox_providers: OutboxProviders,

    /// **Which mail transport this node sends through** (email-transport scope, issue #118) — an SMTP
    /// relay, a provider API, or explicit log-only, selected *by name* so a host gets working email from
    /// configuration alone instead of implementing a trait.
    ///
    /// `None` (the default) keeps prior behaviour — the relay drains through the logging provider — but
    /// boot now **warns loudly** that every email is being dropped, because that silence is exactly what
    /// let issue #118 live: an admin invited a colleague, the outbox drained clean, and the colleague was
    /// never told. `Some(EmailTransport::Logging)` is the same behaviour chosen deliberately, and stays
    /// quiet.
    ///
    /// [`outbox_providers.email`](OutboxProviders::email) still WINS over this: a host that handed us its
    /// own `EmailProvider` meant it. Credentials are never in here — the config carries a secrets PATH and
    /// an env NAME, resolved at send time in the effect's workspace. `from_env` reads `LB_MAIL_*`.
    pub email_transport: Option<crate::mail::EmailTransport>,

    /// Which credential check `POST /auth/login` runs before minting (embedder-credential-mode scope).
    /// Additive: `Default` is [`CredentialMode::DevTrustAny`] — today's `boot_full` password-less
    /// behaviour, so no existing embedder breaks. An embedder sets [`CredentialMode::PasswordHash`]
    /// to enforce real passwords (argon2 against the stored credential); `from_env()` derives it
    /// from `LB_DEV_LOGIN` to reproduce the standalone binary. Applied through
    /// `Gateway::with_global_credential_check` in the builder's `GatewayMode::Addr` arm; irrelevant
    /// when the gateway is `Off` (no login route).
    pub credential_mode: CredentialMode,

    /// Whether the extension **publisher-signature** check is enforced, or waived for a development
    /// node (the `LB_EXT_UNTRUSTED_KEY=allow` escape hatch — see `lb_gateway::session::trusted`).
    ///
    /// Additive and fail-closed: `Default` is [`Authenticity::Required`], today's behaviour exactly,
    /// so no existing embedder changes posture. `from_env()` derives it from `LB_EXT_UNTRUSTED_KEY`
    /// to reproduce the standalone binary. Applied through `Gateway::with_authenticity` in the
    /// builder's `GatewayMode::Addr` arm.
    ///
    /// Waived, this node accepts an extension signed by a key outside its allow-list — on upload and
    /// on a registry pull alike. It does **not** waive the content-digest check, so a corrupt or
    /// tampered artifact is still rejected. A node in this posture reports `trust_gate` on
    /// `GET /health` and warns on every waived artifact. Development boxes only.
    pub authenticity: Authenticity,

    /// The route-scoped body-size ceiling for the `POST /extensions` artifact upload, in bytes
    /// (extension-upload-limit fix). A NATIVE-tier artifact packs a host-target binary (megabytes)
    /// into the signed Artifact's `wasm` field, JSON-encoded as a byte array (~8× the binary), so a
    /// real native sidecar's upload runs to hundreds of MiB — far past axum's 2 MiB `DefaultBodyLimit`
    /// default, which used to 413 ("length limit exceeded") before the body was ever read. This raises
    /// the ceiling ON THAT ONE ROUTE (never a global bump — rule 10), and is deliberately NOT unlimited
    /// so a runaway upload can't OOM the node. Default 384 MiB fits the largest real sidecar artifact
    /// seen (the ems modbus bundle at ~317 MiB) with headroom; an embedder with a bigger binary raises
    /// it. `from_env` reads `LB_MAX_EXTENSION_UPLOAD_BYTES`. Only the `POST /extensions` route reads it.
    pub max_extension_upload_bytes: u64,

    /// An optional password to seed for the dev [`seed_user`](Self::seed_user) at boot, so a node
    /// running [`CredentialMode::PasswordHash`] has a first admin who can actually log in (the
    /// bootstrap-paradox fix: `identity.set_credential` needs an admin token, but no admin can
    /// authenticate until a credential exists). `Some(non-empty)` ⇒ the boot seed argon2-hashes it
    /// into the seed user's credential record (idempotent, alongside the membership seed). `None`
    /// (the default) seeds no credential — correct for `DevTrustAny` nodes (password-less) and for
    /// an embedder that provisions credentials its own way. Secret-class: filled at the binary
    /// boundary, hashed before write, never logged (mirrors `signing_key`'s custody).
    pub seed_credential: Option<String>,

    /// An optional GLOBAL email login handle to seed for the dev [`seed_user`](Self::seed_user) at
    /// boot (email-login scope), so the new `POST /auth/login {email, password}` front door has a
    /// first admin who can sign in on a fresh store. `Some(non-empty)` ⇒ the seed sets the identity's
    /// globally-unique email AND — when [`seed_credential`](Self::seed_credential) is also set — the
    /// global password. `/auth/login` is the ONLY human door (the legacy `POST /login` was deleted in
    /// the pre-production sweep), so `None` here means the seeded dev admin **cannot log in** on a
    /// `PasswordHash` node — set it whenever you set `seed_credential`.
    /// `from_env` reads `LB_SEED_EMAIL`. Filled at the binary boundary; the email is non-secret.
    pub seed_email: Option<String>,

    /// The optional server-side response cache (response-cache scope). `None` (the default) ⇒ no
    /// cache — every read dispatches, byte-for-byte today's behaviour. `Some(CacheConfig)` ⇒ the
    /// node builds an in-process moka cache with that budget/TTL and serves the audited allowlist
    /// (`datasource.list`/`series.list`/`flows.list`/`flows.get`/`ext.list`) read-through +
    /// single-flight, invalidated by per-`{ws, class}` generation counters on write.
    ///
    /// Additive on two axes, matching the scope's "optional twice": the `page-cache` CARGO feature
    /// gates whether the live cache is even compiled (off ⇒ this field is honoured as a no-op, no
    /// `moka` in the binary); this field is the RUNTIME switch on a build that compiled it in. An
    /// embedder fills it directly (rubix-ai maps `RUBIX_CACHE_*` → here at its binary boundary);
    /// `from_env` maps `LB_CACHE_*` for the standalone binary.
    pub cache: Option<lb_host::CacheConfig>,

    /// The durable per-source **discovery profile** (datasource-profile scope). `None` (the default)
    /// ⇒ no freshness reactor runs and nothing is profiled on a clock — byte-for-byte today's
    /// behaviour. `Some(ProfileConfig)` with `enabled` ⇒ the node ticks `react_to_profiles` per
    /// workspace: profiles older than `refresh_after_secs` are re-enqueued as `lb-jobs` jobs and
    /// drained, so a source's shape stays current without any read ever blocking on a pass.
    ///
    /// Additive on the same two axes as [`cache`](Self::cache): the `datasource-profile` CARGO
    /// feature gates whether any of it is COMPILED (off ⇒ the verbs are absent from the catalog and
    /// this field is honoured as a no-op); this field is the RUNTIME switch on a build that
    /// compiled it in. Role = config, never a code branch (rule 1) — a cloud node and an edge node
    /// differ only in these numbers.
    ///
    /// `from_env` maps `LB_PROFILE_*` for the standalone binary; an embedder fills it directly.
    pub profile: Option<ProfileConfig>,

    /// The node's **disk budget** for the store directory, in bytes (disk-budget scope, slice 1).
    /// `None` (the default) ⇒ today's behaviour exactly: the flat
    /// [`lb_host::LOG_ADVISORY_BYTES`] advisory (256 MiB) and **no marks** — nothing derives from a
    /// budget that does not exist, so nothing new can trigger on upgrade (scope decision 2).
    /// `Some(bytes)` ⇒ the advisory threshold and the soft/hard marks derive from it as percentages
    /// ([`lb_host::SOFT_MARK_PCT`] / [`lb_host::HARD_MARK_PCT`], scope decision 1).
    ///
    /// The budget covers the **store directory** (the commit log + manifest), not the partition —
    /// extension artifacts, sidecar binaries and OS logs sharing the filesystem are outside it.
    /// That distinction is what `store.status`'s `free_disk_bytes` exists to keep visible.
    ///
    /// Config, never a code branch (rule 1): an edge node on an SD card and a cloud node on a volume
    /// differ only in the number. An embedder fills the field; `from_env` reads
    /// `LB_STORE_MAX_BYTES` at the binary boundary — the one place `LB_*` is read.
    pub store_budget_bytes: Option<u64>,

    /// Force the store open past the **boot memory guard** (`LB_STORE_OPEN_UNGUARDED=1`, parsed in
    /// [`crate::store_env`]): `false` (default) ⇒ a log larger than available RAM is refused with
    /// `StoreError::WontFit` and the binary exits nonzero rather than risking a machine-wide OOM
    /// (#128). The *open* guard only — the compaction preconditions are not overridable.
    pub store_open_unguarded: bool,

    /// What that guard should treat as this machine's available RAM. `None` (default, and what the
    /// binary always fills) ⇒ read `/proc/meminfo`. An embedder sets it when it knows a truer
    /// ceiling (under a cgroup, `MemoryMax` is the node's real budget). Never read from env.
    pub store_available_ram_bytes: Option<u64>,

    /// Advertise this node on the local network over **mDNS/DNS-SD** so peers can discover an
    /// endpoint to dial before they have a bus session (`lb-discovery`).
    ///
    /// `None` (the default) ⇒ **no advertisement at all**, today's unchanged behaviour: the node is
    /// invisible to mDNS and nothing is broadcast. `Some(ad)` ⇒ the node publishes its id, port and
    /// version under the configured service type for as long as it runs.
    ///
    /// This is the **bootstrap** layer, not the roster: it answers "what lb nodes are on this wire"
    /// for a node that has no endpoint yet, and hands off to Zenoh + the fleet-presence liveliness
    /// roster (`ws/{id}/nodes/{node_id}`), which stays authoritative for workspace presence. The two
    /// compose; neither replaces the other. Zenoh's own multicast scouting already covers the plain
    /// same-subnet case, so this earns its keep where scouting is filtered or where operators want
    /// `avahi-browse`/`dns-sd` to see the fleet.
    ///
    /// **What it broadcasts is reachability only** — never a workspace, persona, capability or
    /// extension list, because an mDNS record is readable by anything on the segment and sits
    /// outside every wall the platform has (rule 6). Discovery is not authorization: the caps wall
    /// and workspace isolation gate every byte after the dial, unchanged.
    ///
    /// The service type is embedder-supplied and product-agnostic (default `_lb._tcp`) — no core
    /// crate names a product (rule 10). Opt-in and non-fatal: if the network refuses mDNS, boot logs
    /// a warning and the node serves normally.
    pub discovery: Option<lb_discovery::Advertisement>,

    /// Who this node is — the addressable id, an optional machine-derived id, and the operator's
    /// human label ([`lb_discovery::NodeIdentity`]).
    ///
    /// `None` (the default) ⇒ unchanged behaviour: the node keeps the fresh random per-process id
    /// `boot*` mints and serves no identity route. `Some(id)` ⇒ the node **installs that id as its
    /// bus identity** (`Node::install_node_id`) and serves it on the unauthenticated `GET /node`.
    ///
    /// Installing it is the point, and it fixes a real gap: `boot*`'s per-boot random id is correct
    /// for a test (unique, never collides) but wrong for a deployment — a restart looks like a
    /// brand-new node and the fleet roster grows ghosts. This is the config seam
    /// `Node::node_id`'s docs have always pointed at.
    ///
    /// **The embedder supplies the values; no core crate derives them** (rule 10). lb does not
    /// reach for `/etc/machine-id`, a board serial, or any other platform-specific source — a
    /// product host knows which is right for its hardware and fills this field at the binary
    /// boundary. That also keeps lb free of a dependency on any one identity crate.
    ///
    /// Note this and [`discovery`](Self::discovery) are independent: a node may serve `GET /node`
    /// without advertising on mDNS, or advertise without serving a gateway. When both are set, an
    /// embedder should build them from the SAME identity so the two surfaces cannot disagree.
    pub identity: Option<lb_discovery::NodeIdentity>,

    /// **How this node replaces itself** (node-update scope §Seam 1) — the `update.*` verb family's
    /// provider, plus the credential's custody NAMES (a secret PATH and an env var NAME, never a
    /// value).
    ///
    /// `None` (the default, and every existing embedder) ⇒ `update.status` answers
    /// `{"supported": false}` and every other `update.*` verb is a clean `Unsupported` — the honest
    /// [`UnconfiguredModel`](lb_host::UnconfiguredModel) posture, never a `404`. Byte-for-byte prior
    /// behaviour: nothing new runs, nothing new is stored, no route changes.
    ///
    /// **lb performs no update and stores no artifact.** The mechanism — a supervisor, an
    /// orchestrator, a package manager — is the embedder's; no core crate names one (rule 10). lb
    /// owns the vocabulary, the caps wall, the audit trail and the credential's custody: the sealed
    /// record is stamped owner = the node host principal in the BOOT workspace, so no principal —
    /// admin included — can read it back through `secret.get`, and (since the raw-read wall)
    /// `store.query`/`scan`/`graph` refuse the `secret` table structurally too.
    ///
    /// Placement-agnostic, like every other field here: a cloud node behind an orchestrator and an
    /// edge node under a supervisor fill this ONE seam with different providers — role is config,
    /// never a code branch (rule 2). Deliberately NOT read from env: a provider is a trait object,
    /// so there is nothing for `from_env` to parse, and the binary leaves it `None`.
    pub update: Option<lb_host::UpdateConfig>,

    /// **Where big binary uploads land** (node-update scope §Seam 2) — embedder-registered upload
    /// sinks keyed by an OPAQUE name, exactly the posture
    /// [`OutboxProviders::targets`](OutboxProviders::targets) already set.
    ///
    /// Empty (the default, and every existing embedder) ⇒ the `/uploads/*` gateway routes are **not
    /// mounted at all**; unmatched paths answer exactly as they do today. Non-empty ⇒ the gateway
    /// serves a resumable, chunked lane per sink: `POST /uploads/{sink}` to open, `PATCH` with a
    /// `Content-Range` to stream, `GET` for the offset, `POST …/complete`, `DELETE` to abort.
    ///
    /// **Bytes stream, never buffer, and resume.** The request body is read as a stream and handed
    /// to the sink in 64 KiB pieces; lb never collects a body and never writes an artifact to its own
    /// disk, so peak memory is one chunk regardless of artifact size and an interrupted 2 GB upload
    /// continues from its offset. Offsets are the SINK's truth (lb holds no upload state, so
    /// resumption survives an lb restart for free), and the resume identity is `digest_hex`.
    ///
    /// Each route enforces the sink's OWN `required_cap()` — the sink chooses, lb enforces, on every
    /// call in the sequence. The core names no sink: a sink called `"package"` on one host and
    /// `"firmware"` on another needs zero lb change (rule 10). Not read from env, for the same reason
    /// [`update`](Self::update) is not.
    pub upload_sinks: Vec<(String, std::sync::Arc<dyn lb_host::UploadSink>)>,
}

impl Default for BootConfig {
    /// The embed-friendly default: `mem://` store, a fresh signing key, workspace `acme`, dev-identity
    /// seed on, gateway OFF, reactors ON, **hello demo OFF** (an embedder does not want it), default
    /// core skills, no telemetry, an unconfigured agent model. Note `hello_demo` is `false` here but
    /// `from_env()` sets it `true` to reproduce today's binary exactly.
    fn default() -> Self {
        Self {
            store_path: None,
            // The boot memory guard is ON by default for every embedder: it only ever fires on a
            // store this machine provably cannot replay, and the failure it replaces is a
            // machine-wide OOM (boot-memory-guard scope, issue #128).
            store_open_unguarded: false,
            store_available_ram_bytes: None,
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
            // `None` ⇒ no static-root fallback (unmatched paths 404, today's behaviour); an embedder
            // sets a dir to serve a self-contained web app at `/`.
            static_root: None,
            // `None` ⇒ bearer-only, no `/api/*` seam, no cookies (today's behaviour). An embedder
            // whose shell lb serves opts in (browser-session scope).
            browser_session: None,
            outbox_providers: OutboxProviders::default(),
            // No mail transport by default — an embedder selects one (or hands in its own provider).
            // Boot warns that email is being dropped; it does not crash (never strand effects).
            email_transport: None,
            // Back-compat embed default: password-less. An embedder opts into `PasswordHash`
            // explicitly; `from_env()` (below) derives it from `LB_DEV_LOGIN` for the binary.
            credential_mode: CredentialMode::DevTrustAny,
            // Fail closed. Unlike `credential_mode` above, the embed default here is the STRICT one:
            // a weakened publisher gate must never be something an embedder gets by not asking.
            authenticity: Authenticity::Required,
            // The route-scoped upload ceiling for `POST /extensions` (default 384 MiB). Additive:
            // a bounded, embedder-overridable limit that fits a real native sidecar artifact.
            max_extension_upload_bytes: DEFAULT_MAX_EXTENSION_UPLOAD_BYTES,
            // No seed password by default — a `DevTrustAny` node needs none, and an embedder fills
            // this only when it boots `PasswordHash` and wants the dev admin to be able to log in.
            seed_credential: None,
            // No global email seeded by default — an embedder fills this to give the `/auth/login`
            // front door a first admin. `None` ⇒ the seeded dev user has no way to sign in.
            seed_email: None,
            // No response cache by default — an embedder opts in (rubix-ai does, on by default in
            // ITS binary). `from_env` (below) turns it on for the standalone binary via `LB_CACHE_*`.
            cache: None,
            // No profiling by default (the `cache` posture): a node must not silently start
            // spending work on an external database because it was upgraded. An embedder opts in;
            // `from_env` (below) reads `LB_PROFILE*` for the standalone binary.
            profile: None,
            // No disk budget by default (disk-budget scope decision 2): unset means today's flat
            // 256 MiB advisory and no marks, forever. No auto-derivation from filesystem size — a
            // node must not silently acquire a new behaviour on upgrade.
            store_budget_bytes: None,
            // `None` ⇒ the node advertises NOTHING on the LAN (today's behaviour). Opting a node
            // into being discoverable is an explicit act by the embedder, never a default: a
            // network broadcast that switches itself on at upgrade is exactly the surprise this
            // posture avoids.
            discovery: None,
            identity: None,
            // `None` ⇒ this node cannot replace itself: `update.status` answers
            // `{"supported": false}` and every other verb is a clean `Unsupported`. An embedder
            // fills it with its own provider; no core crate names a mechanism (rule 10).
            update: None,
            // Empty ⇒ the `/uploads/*` routes are not mounted at all. An embedder registers its
            // sinks; the gateway never learns whose bytes it moves.
            upload_sinks: Vec::new(),
        }
    }
}

impl BootConfig {
    /// Read the boot config from `LB_*` env, reproducing today's `node` binary behaviour EXACTLY. This
    /// is the ONLY place boot env vars are read; only binaries call it. Embedders construct [`BootConfig`]
    /// directly and never touch env.
    pub fn from_env() -> Self {
        // Computed once: discovery advertises the port peers should DIAL, which is the gateway's.
        // Deriving it here keeps the two from drifting — a node that advertised a port it does not
        // serve on would be discoverable and unreachable, the worst of both.
        let gateway = gateway_mode_from_env();
        // ONE identity feeds BOTH public surfaces — the mDNS advertisement and `GET /node`. Built
        // once here and handed to both for the same reason the gateway port is: two independently
        // derived identities would be free to disagree, and a node whose broadcast id differs from
        // the one it serves over HTTP is worse than a node with no identity at all.
        let identity = identity_from_env();
        let discovery = discovery_from_env(&gateway, identity.clone());
        BootConfig {
            store_path: std::env::var("LB_STORE_PATH")
                .ok()
                .filter(|p| !p.is_empty()),
            signing_key: gateway_signing_key(),
            workspace: std::env::var("LB_WORKSPACE").unwrap_or_else(|_| "acme".into()),
            seed_user: Some(std::env::var("LB_SEED_USER").unwrap_or_else(|_| "user:ada".into())),
            gateway,
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
            // The static-root fallback is a NEW seam with no gateway-internal env read (unlike
            // `ext_ui_dir`), so the standalone binary honours `LB_STATIC_ROOT` here: set it to serve a
            // self-contained web app at `/`; unset ⇒ `None` ⇒ no fallback (unchanged 404 behaviour).
            static_root: std::env::var("LB_STATIC_ROOT")
                .ok()
                .filter(|p| !p.is_empty()),
            // The `/api/*` browser-session seam, off unless asked for (browser-session scope). Same
            // posture as `static_root`: a new seam with no gateway-internal env read, so the
            // standalone binary honours `LB_BROWSER_SESSION` here. Set it (any non-empty value) to
            // terminate cookie sessions for a served shell; unset ⇒ `None` ⇒ bearer-only, unchanged.
            // `LB_BROWSER_SESSION_SECURE` marks the cookie `Secure` (TLS deploys) — off by default so
            // the plain-http LAN/Pi deploys this seam exists for are not silently broken by a cookie
            // the browser drops.
            browser_session: std::env::var("LB_BROWSER_SESSION")
                .ok()
                .filter(|v| !v.is_empty())
                .map(|_| {
                    // `default()`-then-mutate: `BrowserSessionConfig` is `#[non_exhaustive]`, so a
                    // cross-crate struct literal is (deliberately) forbidden — that is what keeps a
                    // future field additive for every embedder.
                    let mut c = lb_role_gateway::BrowserSessionConfig::default();
                    c.secure_cookie =
                        std::env::var("LB_BROWSER_SESSION_SECURE").is_ok_and(|v| !v.is_empty());
                    c
                }),
            // The binary configures no real delivery providers today — the relay drains through
            // the logging no-ops. Real adapters come from an embedder filling the struct.
            outbox_providers: OutboxProviders::default(),
            // The standalone binary reads its mailer from `LB_MAIL_*` (the binary boundary is the ONE
            // place env is read). Unset ⇒ None ⇒ the loud "email is being dropped" boot warning.
            email_transport: crate::mail::email_transport_from_env(),
            // Mirror the standalone gateway's `credential_check_from_env`: `LB_DEV_LOGIN` set/
            // non-empty ⇒ `DevTrustAny` (password-less dev/CI), unset ⇒ `PasswordHash` (the real
            // argon2 check). This reproduces the `node` binary exactly — the binary's default is
            // password-enforcing, unlike the embed `Default` which stays `DevTrustAny` for
            // back-compat.
            credential_mode: credential_mode_from_env(),
            // The publisher trust gate from `LB_EXT_UNTRUSTED_KEY`: only the exact token `allow`
            // waives the signature check; unset/empty/anything else ⇒ `Required` (today's behaviour).
            // The gateway's parser warns on stderr both when it disables the gate and when it
            // ignores an unrecognised value. Read only here, at the binary boundary.
            authenticity: lb_role_gateway::authenticity_from_env(),
            // The `POST /extensions` upload ceiling from `LB_MAX_EXTENSION_UPLOAD_BYTES` (bytes);
            // unset/empty/unparseable ⇒ the 384 MiB default. Read only here, at the binary boundary.
            max_extension_upload_bytes: max_extension_upload_bytes_from_env(),
            // The node's store disk budget from `LB_STORE_MAX_BYTES` (bytes); unset/empty/
            // unparseable ⇒ `None` ⇒ today's flat 256 MiB advisory and no marks. Read only here.
            store_budget_bytes: store_budget_bytes_from_env(),
            // The boot memory-guard override from `LB_STORE_OPEN_UNGUARDED` (exactly `1`);
            // anything else warns and leaves the guard on. Read only here, at the binary boundary —
            // the store crate reads no env and takes this as a parameter.
            store_open_unguarded: store_open_unguarded_from_env(),
            // The binary measures the machine (`/proc/meminfo`); only an embedder overrides it.
            store_available_ram_bytes: None,
            // LAN discovery from `LB_DISCOVERY_*` — OFF unless `LB_DISCOVERY=1`, so the standalone
            // binary broadcasts nothing until an operator asks for it. Read only here.
            discovery,
            // The durable node identity (`LB_NODE_*`) — installed as the bus id and served on the
            // unauthenticated `GET /node`. Shared with `discovery` above so the two agree.
            identity,
            // Optional dev-admin seed password (`LB_SEED_PASSWORD`) — so a `PasswordHash` binary
            // has a first admin who can log in. Absent ⇒ no credential seeded (correct for a
            // `DevTrustAny` binary). Secret-class: read here, hashed at seed time, never logged.
            seed_credential: std::env::var("LB_SEED_PASSWORD")
                .ok()
                .filter(|s| !s.is_empty()),
            // Optional dev-admin GLOBAL email (`LB_SEED_EMAIL`) — so the new `/auth/login` front door
            // has a first admin. Absent ⇒ no global email seeded. Read here at the binary boundary.
            seed_email: std::env::var("LB_SEED_EMAIL")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            // The standalone binary keeps the cache OFF unless asked (lb does not put `page-cache`
            // in its default features, so the stock binary usually can't cache anyway). `LB_CACHE=1`
            // (or any non-empty non-"0") turns it on; `LB_CACHE_BUDGET_MB` sizes it. Read only here.
            cache: cache_from_env(),
            // OFF unless asked, same posture as the cache — and doubly so, because this one spends
            // work on someone else's database. `LB_PROFILE=1` turns it on. Read only here.
            profile: profile_from_env(),
            // Deliberately absent from the env path: both seams carry TRAIT OBJECTS, so there is
            // nothing for a binary-boundary reader to parse. The standalone `node` binary ships with
            // no update provider and no upload sinks — an embedder fills the struct directly.
            update: None,
            upload_sinks: Vec::new(),
        }
    }
}

/// The standalone binary's discovery-profile config from `LB_PROFILE*` (from-env only). `None`
/// unless `LB_PROFILE` is truthy. Embedders never call this — they fill `BootConfig.profile`
/// directly. Env is a BINARY concern: nothing below the boot seam reads it.
fn profile_from_env() -> Option<ProfileConfig> {
    let on = std::env::var("LB_PROFILE")
        .ok()
        .map(|v| !v.is_empty() && v != "0")
        .unwrap_or(false);
    if !on {
        return None;
    }
    let d = ProfileConfig::default();
    let num = |k: &str, fallback: u64| {
        std::env::var(k)
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(fallback)
    };
    Some(ProfileConfig {
        enabled: true,
        refresh_after_secs: num("LB_PROFILE_REFRESH_SECS", d.refresh_after_secs),
        max_tables: num("LB_PROFILE_MAX_TABLES", d.max_tables),
        max_values: num("LB_PROFILE_MAX_VALUES", d.max_values),
    })
}

/// The standalone binary's response-cache config from `LB_CACHE*` (from-env only). `None` unless
/// `LB_CACHE` is set to a truthy value; the budget comes from `LB_CACHE_BUDGET_MB` (default 32).
/// Embedders never call this — they fill `BootConfig.cache` directly (rubix-ai maps `RUBIX_CACHE_*`).
fn cache_from_env() -> Option<lb_host::CacheConfig> {
    let on = std::env::var("LB_CACHE")
        .ok()
        .map(|v| !v.is_empty() && v != "0")
        .unwrap_or(false);
    if !on {
        return None;
    }
    let mut c = lb_host::CacheConfig::default();
    if let Some(mb) = std::env::var("LB_CACHE_BUDGET_MB")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        c.memory_budget_bytes = mb * 1024 * 1024;
    }
    Some(c)
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

/// Build the node's durable identity from `LB_NODE_*` at the binary boundary.
///
/// This is the standalone binary's answer to the gap `Node::node_id`'s docs describe: without it a
/// node runs the fresh-per-process random id `boot*` mints, so every restart looks like a brand-new
/// node and the fleet roster grows ghosts. Setting `LB_NODE_ID` makes the identity durable.
///
/// - `LB_NODE_ID` — the addressable id (default `node:<hostname>`). Must be key-expression-safe: it
///   is interpolated into bus keys (`ws/{id}/nodes/{node}`), so an id containing `/` or `*` could
///   change a key's SHAPE — a `*` would silently address the whole fleet. Invalid ⇒ `None` (no
///   durable identity installed, and LAN discovery stays off), never a silent fallback: quietly
///   substituting a different id is how a node ends up addressable under an id nobody configured.
/// - `LB_NODE_NAME` — the operator's human label (default: the id). Display text, never an
///   identifier; nothing addresses or routes by it.
/// - `LB_NODE_MACHINE_ID` — an opaque machine-derived id, absent by default. The binary does NOT
///   derive one: reading `/etc/machine-id` or a board serial is platform-specific and belongs to
///   the product host, not to lb (rule 10). Whatever is passed is published in cleartext over mDNS
///   and on `GET /node`, so pass a hashed form if it originated as an OS machine-id.
fn identity_from_env() -> Option<lb_discovery::NodeIdentity> {
    // `LB_DISCOVERY_NODE_ID` is the ORIGINAL name of this var, from when the id was
    // discovery-specific. It is now the node's identity everywhere (bus id, `GET /node`, mDNS), so
    // `LB_NODE_ID` is the name that describes it — but the old one keeps working, because silently
    // ignoring an operator's configured id would re-address a deployed node on upgrade. `LB_NODE_ID`
    // wins when both are set.
    let (var, node_id) = std::env::var("LB_NODE_ID")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|v| ("LB_NODE_ID", v))
        .or_else(|| {
            std::env::var("LB_DISCOVERY_NODE_ID")
                .ok()
                .filter(|s| !s.is_empty())
                .map(|v| ("LB_DISCOVERY_NODE_ID", v))
        })
        .unwrap_or_else(|| {
            let host = std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".into());
            ("LB_NODE_ID", format!("node:{host}"))
        });
    let node = match lb_bus::NodeId::new(node_id.clone()) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("bad {var} '{node_id}': {e} — this node has NO durable identity");
            return None;
        }
    };

    let mut identity = lb_discovery::NodeIdentity::new(node);
    // `with_name` ignores a blank, so an empty var keeps the id-derived default.
    if let Ok(name) = std::env::var("LB_NODE_NAME") {
        identity = identity.with_name(name);
    }
    if let Some(mid) = std::env::var("LB_NODE_MACHINE_ID")
        .ok()
        .filter(|s| !s.is_empty())
    {
        identity = identity.with_machine_id(mid);
    }
    Some(identity)
}

/// Build the LAN-discovery advertisement from `LB_DISCOVERY_*`, or `None` to advertise nothing.
///
/// **Off unless `LB_DISCOVERY=1`.** A node broadcasting its existence on the local network is a
/// posture change an operator must ask for; it must never arrive as a silent default on upgrade.
///
/// Requires a gateway: discovery advertises an endpoint to *dial*, and a headless node has none, so
/// a headless node with `LB_DISCOVERY=1` warns and stays silent rather than advertising a port that
/// refuses connections.
///
/// The id, name and machine id come from the SHARED identity (`LB_NODE_*`, see
/// [`identity_from_env`]) rather than from discovery-specific vars, so the advertisement and
/// `GET /node` cannot disagree about who this node is.
/// - `LB_DISCOVERY_SERVICE_TYPE` — the DNS-SD type (default `_lb._tcp`); a product host sets its own.
/// - `LB_DISCOVERY_FLEET` — an opaque grouping tag so unrelated fleets on one LAN ignore each other.
///   Plaintext on the wire and trivially forged — it separates accidents, not adversaries.
fn discovery_from_env(
    gateway: &GatewayMode,
    identity: Option<lb_discovery::NodeIdentity>,
) -> Option<lb_discovery::Advertisement> {
    if std::env::var("LB_DISCOVERY").ok().as_deref() != Some("1") {
        return None;
    }

    let GatewayMode::Addr(addr) = gateway else {
        eprintln!(
            "LB_DISCOVERY=1 but no LB_GATEWAY_ADDR — a headless node has no endpoint to advertise; \
             LAN discovery stays OFF"
        );
        return None;
    };

    // The shared identity (`identity_from_env`) IS the advertised one — see `from_env`. It is
    // `None` only when the operator gave an invalid `LB_NODE_ID`, which that reader already
    // reported; advertising a fallback id after refusing to install it would be the exact
    // disagreement between surfaces this sharing exists to prevent.
    let identity = identity?;

    let mut ad = lb_discovery::Advertisement::with_identity(identity, addr.port());

    if let Some(ty) = std::env::var("LB_DISCOVERY_SERVICE_TYPE")
        .ok()
        .filter(|s| !s.is_empty())
    {
        match lb_discovery::ServiceType::new(ty.clone()) {
            Ok(t) => ad.service_type = t,
            Err(e) => {
                eprintln!("bad LB_DISCOVERY_SERVICE_TYPE '{ty}': {e} — LAN discovery stays OFF");
                return None;
            }
        }
    }
    ad.version = Some(env!("CARGO_PKG_VERSION").to_string());
    ad.fleet = std::env::var("LB_DISCOVERY_FLEET")
        .ok()
        .filter(|s| !s.is_empty());

    Some(ad)
}

/// Parse `LB_MAX_EXTENSION_UPLOAD_BYTES` (a plain byte count) into the `POST /extensions` upload
/// ceiling; unset/empty/unparseable ⇒ [`DEFAULT_MAX_EXTENSION_UPLOAD_BYTES`] (384 MiB). A malformed
/// value warns and falls back rather than panicking (the "don't panic in boot config" posture). Only
/// this binary-boundary reader touches the env — the value rides `BootConfig` below the boot seam.
fn max_extension_upload_bytes_from_env() -> u64 {
    match std::env::var("LB_MAX_EXTENSION_UPLOAD_BYTES") {
        Ok(v) if !v.trim().is_empty() => v.trim().parse().unwrap_or_else(|_| {
            eprintln!(
                "bad LB_MAX_EXTENSION_UPLOAD_BYTES '{v}': not a byte count — using the {} MiB default",
                DEFAULT_MAX_EXTENSION_UPLOAD_BYTES / (1024 * 1024)
            );
            DEFAULT_MAX_EXTENSION_UPLOAD_BYTES
        }),
        _ => DEFAULT_MAX_EXTENSION_UPLOAD_BYTES,
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
