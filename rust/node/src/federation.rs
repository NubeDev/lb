//! The `federation` (datasources) role wiring — env-gated, mounted from `main.rs`. The same thin
//! role-aware layer §3.1 permits in the *binary* as `github.rs`: no core crate is role-aware; the
//! decision to install the native federation sidecar (and pre-approve its `net:*` endpoints) lives
//! here, keyed off config (env), never an `if cloud`.
//!
//! Driven by one env var:
//!   - `LB_FEDERATION_ENDPOINTS` — a comma-separated list of `host:port` endpoints the admin approves
//!     for the federation extension to connect to (`net:tls:host:port:connect` each). Setting it
//!     installs + supervises the `federation` sidecar in `LB_WORKSPACE` with exactly that grant.
//!
//! Optionally, one source can be pre-registered so the Datasources page shows a working entry on
//! first boot (the demo seed against the dev TimescaleDB):
//!   - `LB_FEDERATION_SEED_NAME`     — the datasource alias (e.g. `timescale`).
//!   - `LB_FEDERATION_SEED_KIND`     — the source kind (`postgres` | `timescale` | `sqlite`).
//!   - `LB_FEDERATION_SEED_ENDPOINT` — the `host:port` (MUST be one of `LB_FEDERATION_ENDPOINTS`).
//!   - `LB_FEDERATION_SEED_DSN`      — the libpq DSN; mediated into `lb-secrets`, never logged/returned.
//!
//! The sidecar binary is resolved from the workspace target dir (where `cargo run` builds it); the
//! manifest is the extension's own `extension.toml`. `now` enters here, at the binary boundary, as
//! wall-clock seconds (the no-wall-clock rule keeps time out of the *core crates*).

use std::path::PathBuf;
use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{datasource_add, install_native, Node};
use lb_supervisor::OsLauncher;

/// Wall-clock seconds since the Unix epoch — the install's `now` at the binary boundary.
fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The admin service principal the federation install acts as in `ws` — holds exactly the native
/// install gate, the secret-write (for the seed DSN), and the datasource-add caps. (A real
/// login→token→principal session replaces this demo identity later, like the gateway's dev login.)
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
    };
    verify(&key, &mint(&key, &claims), 1).expect("freshly minted token verifies")
}

/// The federation extension manifest (compiled in so the binary needs no file at this path at run
/// time — it is the same source the E2E test installs from).
const MANIFEST: &str = include_str!("../../extensions/federation/extension.toml");

/// Resolve the directory holding the built `federation` binary (the workspace target dir). `cargo run`
/// builds debug; a release run uses release. Overridable with `LB_FEDERATION_DIR`.
fn federation_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("LB_FEDERATION_DIR") {
        return PathBuf::from(dir);
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // node/ is a workspace member; the shared target/ is one level up.
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    manifest_dir.join("..").join("target").join(profile)
}

/// Mount the federation role on `node` per the environment. Installs + supervises the `federation`
/// sidecar with the admin-approved `net:*` grant, then (optionally) pre-registers one seed source so
/// the Datasources page works on first boot. A no-op if `LB_FEDERATION_ENDPOINTS` is unset.
pub async fn mount(node: Arc<Node>) {
    let Ok(endpoints) = std::env::var("LB_FEDERATION_ENDPOINTS") else {
        return; // The federation role is not configured — no datasources sidecar.
    };
    let endpoints: Vec<String> = endpoints
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if endpoints.is_empty() {
        return;
    }

    let ws = std::env::var("LB_WORKSPACE").unwrap_or_else(|_| "acme".into());
    let admin = admin_principal(&ws);
    let now = unix_seconds();

    // The admin-approved grant: one `net:tls:host:port:connect` per endpoint + the secret read the
    // host needs to mediate a DSN to the pool. `requested ∩ admin_approved` is computed in
    // `install_native`; here we approve exactly the configured endpoints (the per-endpoint wall).
    let mut approved: Vec<String> = endpoints
        .iter()
        .filter_map(|e| e.rsplit_once(':'))
        .map(|(host, port)| format!("net:tls:{host}:{port}:connect"))
        .collect();
    approved.push("secret:federation/*:get".to_string());

    let dir = federation_dir();
    let dir_str = dir.to_string_lossy().into_owned();
    let bin = dir.join("federation");
    if !bin.exists() {
        eprintln!(
            "federation: sidecar binary not found at {} — build it with \
             `cargo build -p federation --features postgres` (skipping install)",
            bin.display()
        );
        return;
    }

    match install_native(
        &node,
        &OsLauncher,
        &admin,
        &ws,
        MANIFEST,
        &dir_str,
        &approved,
        now,
    )
    .await
    {
        Ok(s) => println!(
            "federation: installed sidecar in '{ws}' (tools={:?}, granted={:?}, approved endpoints={:?})",
            s.tools, s.granted_caps, endpoints
        ),
        Err(e) => {
            eprintln!("federation: sidecar install failed: {e}");
            return;
        }
    }

    // Optional seed: pre-register one source so the Datasources page has a working entry on first
    // boot. The DSN is mediated into lb-secrets (only the ref lands on the record).
    if let (Ok(name), Ok(kind), Ok(endpoint)) = (
        std::env::var("LB_FEDERATION_SEED_NAME"),
        std::env::var("LB_FEDERATION_SEED_KIND"),
        std::env::var("LB_FEDERATION_SEED_ENDPOINT"),
    ) {
        let dsn = std::env::var("LB_FEDERATION_SEED_DSN").ok();
        match datasource_add(
            &node,
            &admin,
            &ws,
            &name,
            &kind,
            &endpoint,
            None,
            dsn.as_deref(),
            now,
        )
        .await
        {
            Ok(()) => {
                println!("federation: seeded datasource '{name}' ({kind} @ {endpoint}) in '{ws}'")
            }
            // An already-registered source (a re-run on a persistent store) is fine — not fatal.
            Err(e) => eprintln!("federation: seed datasource '{name}' skipped: {e}"),
        }
    }
}
