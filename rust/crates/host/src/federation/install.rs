//! `install_federation` — the shared install-and-seed helper both the `node` binary
//! (`node/src/federation.rs`) and the standalone desktop `full` boot (`ui/src-tauri/src/full.rs`)
//! call to bring up the datasources sidecar (desktop-federation-bundle scope). It exists so the
//! security-sensitive install (the admin bootstrap principal, the `requested ∩ admin_approved` grant
//! computation via `install_native`, the child token mint) lives in ONE place — the copy-paste
//! alternative was rejected because two copies of a grant/token path drift, and that drift is exactly
//! the class of bug this scope follows (the `full.rs` twin had already dropped a seeder its
//! `node/main.rs` original carried).
//!
//! **CLAUDE §10 — core stays extension-agnostic.** This helper names no extension. It takes the
//! `manifest_toml`, the `admin_approved` grant, and an optional `seed` source as **opaque data**: the
//! caller (the *binary*, where §3.1 permits role-aware wiring) supplies the federation-specific
//! values (the `include_str!`'d manifest, the approved `net:*`/`secret:*` endpoints). Swap federation
//! for an equivalent native datasources extension and only the *binary's* inputs change, never this
//! code — nothing here branches on `id == "federation"`. The one non-generic assumption is that the
//! seed step registers a *datasource* (it calls `datasource_add`); a caller that passes `seed: None`
//! uses this as a plain "install this native manifest with this grant" helper.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_supervisor::Launcher;

use super::add::datasource_add;
use super::error::FederationError;
use crate::boot::Node;
use crate::native::install_native;

/// A source to pre-register after the sidecar is installed, so the Datasources page has a working
/// entry on first boot (the desktop demo `sqlite` db, or the dev `LB_FEDERATION_SEED_*` source). All
/// fields are opaque to this helper; `endpoint` MUST be permitted by the `approved` grant or the
/// source registers but every `test`/`query` against it is refused pre-connect (`enforce_endpoint`).
pub struct SeedSource<'a> {
    pub name: &'a str,
    pub kind: &'a str,
    pub endpoint: &'a str,
    /// The connection string (a sqlite file path, a libpq DSN). Mediated into `lb-secrets` under the
    /// stable `ext:federation` owner by `datasource_add`; never lands on the record, a log, or a
    /// response (§6.7). `None` registers the source with no stored secret.
    pub dsn: Option<&'a str>,
}

/// The admin service principal the install acts as in `ws`: holds exactly the native-install gate,
/// the secret-write (to mediate a seed DSN), and the datasource-add cap — the minimal set the two
/// steps below need. A freshly minted, self-signed token (the same demo-identity shape
/// `node/src/federation.rs` used before this helper); a real login→token session replaces it later.
fn admin_principal(ws: &str) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "ext:federation-bootstrap".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: vec![
            "mcp:native.install:call".into(),
            "mcp:datasource.add:call".into(),
            "secret:federation/*:write".into(),
        ],
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("freshly minted token verifies")
}

/// What the install produced (for the caller to log): the granted caps and the child's tools.
pub struct Installed {
    pub tools: Vec<String>,
    pub granted_caps: Vec<String>,
}

/// Install `manifest_toml`'s native sidecar in `ws` with the admin-approved `approved` grant, then
/// (if `seed` is `Some`) pre-register that source. `install_dir` resolves the binary; `ts` is the
/// injected wall-clock second (time enters at the *binary* boundary — the no-wall-clock rule keeps it
/// out of core, so callers pass it). Returns the install summary; a seed failure is logged by the
/// caller, not fatal (an already-registered source on a re-run is fine).
///
/// The grant computation (`requested ∩ approved`) and the deny wall (`enforce_endpoint` reads the
/// persisted grant) are unchanged from `install_native`/`net.rs`; this only bundles the bootstrap
/// principal + the seed so a binary calls one function instead of re-implementing both.
pub async fn install_federation<L: Launcher>(
    node: &Node,
    launcher: &L,
    ws: &str,
    manifest_toml: &str,
    install_dir: &str,
    approved: &[String],
    seed: Option<SeedSource<'_>>,
    ts: u64,
) -> Result<Installed, FederationError> {
    let admin = admin_principal(ws);

    let supervised = install_native(
        node,
        launcher,
        &admin,
        ws,
        manifest_toml,
        install_dir,
        approved,
        ts,
    )
    .await
    .map_err(|e| FederationError::BadInput(format!("native install: {e}")))?;

    if let Some(s) = seed {
        // Best-effort at the helper level too: surface the error to the caller (which logs it), but
        // an already-registered source (a persistent-store re-run) is not an install failure — the
        // sidecar is up regardless. The caller decides how loud to be.
        datasource_add(
            node, &admin, ws, s.name, s.kind, s.endpoint, None, s.dsn, ts,
        )
        .await?;
    }

    Ok(Installed {
        tools: supervised.tools,
        granted_caps: supervised.granted_caps,
    })
}
