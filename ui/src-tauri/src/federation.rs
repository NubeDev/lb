//! Desktop `full` boot: bring up the bundled **federation** datasources sidecar so a double-clicked
//! standalone `.exe` can register AND query a local sqlite source out of the box
//! (desktop-federation-bundle scope). Without this, `datasource.add` succeeds over the loopback
//! gateway but every `datasource.test` / `federation.query` is refused pre-connect — the sidecar
//! that serves those verbs, and the `net:*` install-grant `enforce_endpoint` checks against, are
//! both absent. This is the desktop analogue of `node/src/federation.rs` (which is env-driven for
//! `make dev`); here the manifest + approved endpoints are the desktop DEFAULT, not read from env.
//!
//! §3.1 permits this role-aware wiring in the *binary*. The shared install itself
//! (`lb_host::install_federation`) stays extension-agnostic — it takes the manifest + grant + seed
//! as opaque data; this file supplies the federation-specific values (the compiled-in manifest, the
//! sqlite-loopback grant, the demo source). One responsibility (FILE-LAYOUT §8): mount federation on
//! the standalone desktop node.
//!
//! **Desktop default = sqlite only.** The approved grant permits exactly `127.0.0.1:0` (the local
//! sqlite endpoint convention — sqlite has no network endpoint) + the DSN secret read. A postgres
//! source at a real `host:port` still registers, but its every probe is refused until an admin
//! widens the grant (deferred: the scope's postgres-in-desktop open question). No arbitrary outbound
//! network from a shipped desktop app by default.

use std::path::PathBuf;
use std::sync::Arc;

use lb_host::{install_federation, Node, OsLauncher, SeedSource};

/// The federation extension manifest, compiled in so the packaged binary needs no file on disk at
/// this path at run time (it is the same source `node/src/federation.rs` and the E2E test install
/// from). The *binary* naming "federation" here is what §3.1 permits; the host helper it feeds stays
/// generic.
const MANIFEST: &str = include_str!("../../../rust/extensions/federation/extension.toml");

/// The local sqlite endpoint convention: sqlite has no network endpoint, so a source registers under
/// the sentinel `127.0.0.1:0` and the install grant approves exactly that. This is the ONE endpoint
/// the desktop default trusts (the deny wall stays closed for anything else).
const SQLITE_ENDPOINT: &str = "127.0.0.1:0";

/// The bundled demo datasource alias (the seeded `demo-buildings.db`). Skippable via
/// `LB_DESKTOP_NO_DEMO_SOURCE=1` for a user who wants a clean workspace (mirrors the seed toggles).
const DEMO_SOURCE_NAME: &str = "demo-buildings";

/// Resolve the directory the packaged sidecar binary + demo db sit in — beside the shell's own exe
/// (packaging copies `federation`/`federation.exe` and `demo-buildings.db` there). Overridable with
/// `LB_FEDERATION_DIR` (the same escape hatch `node/src/federation.rs` uses) for a dev run where the
/// sidecar lives in the workspace target dir. Falls back to the current dir if the exe path can't be
/// read (best-effort — the install below just won't find the binary and prints, never panics).
fn sidecar_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("LB_FEDERATION_DIR") {
        return PathBuf::from(dir);
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Mount + supervise the bundled federation sidecar on `node` in `ws` with the sqlite-loopback grant,
/// then pre-register the bundled `demo-buildings.db` so the Datasources page has a green, queryable
/// entry on first boot. Best-effort, LOUD: an install/seed failure prints and returns — the app
/// still opens, datasources just stay red — so the failure can't silently reproduce the "why is my
/// datasource denied" confusion this scope fixes. Idempotent (LWW records), re-run every boot.
///
/// MUST be called AFTER the gateway installs the node signing key (`Gateway::new_live`), so the
/// child's minted `LB_EXT_TOKEN` is signed with the key the gateway verifies callbacks against — the
/// same ordering `node/main.rs` uses.
pub async fn mount_federation(node: Arc<Node>, ws: &str, ts: u64) {
    let dir = sidecar_dir();
    let sidecar_name = if cfg!(windows) {
        "federation.exe"
    } else {
        "federation"
    };
    let bin = dir.join(sidecar_name);
    if !bin.exists() {
        eprintln!(
            "full: federation sidecar not found at {} — datasources will be unavailable \
             (build/package it beside the shell, or set LB_FEDERATION_DIR). Skipping.",
            bin.display()
        );
        return;
    }
    let dir_str = dir.to_string_lossy().into_owned();

    // The desktop default grant: the local sqlite endpoint + the DSN secret read. `requested ∩
    // approved` is computed in the helper; approving only `127.0.0.1:0` is the wall that keeps a
    // postgres source at a real host:port refused (deny path) while the sqlite demo passes.
    let approved = vec![
        format!("net:tls:{SQLITE_ENDPOINT}:connect"),
        "secret:federation/*:get".to_string(),
    ];

    // Pre-register the bundled demo db unless opted out. The DSN is the on-disk path (resolved beside
    // the exe, the same dir the packaging copied it to); mediated into lb-secrets by the helper, only
    // the ref lands on the record (§6.7). A missing file is not fatal here — the source still
    // registers; its `test` then reports the file-not-found honestly.
    let demo_db = dir.join("demo-buildings.db");
    let demo_path = demo_db.to_string_lossy().into_owned();
    let skip_demo = std::env::var("LB_DESKTOP_NO_DEMO_SOURCE")
        .map(|v| v == "1")
        .unwrap_or(false);
    let seed = if skip_demo {
        None
    } else {
        Some(SeedSource {
            name: DEMO_SOURCE_NAME,
            kind: "sqlite",
            endpoint: SQLITE_ENDPOINT,
            dsn: Some(&demo_path),
        })
    };

    match install_federation(
        &node,
        &OsLauncher,
        ws,
        MANIFEST,
        &dir_str,
        &approved,
        seed,
        ts,
    )
    .await
    {
        Ok(s) => {
            println!(
                "full: installed federation sidecar in '{ws}' (tools={:?}, granted={:?}, approved={SQLITE_ENDPOINT})",
                s.tools, s.granted_caps
            );
            if !skip_demo {
                println!(
                    "full: pre-registered datasource '{DEMO_SOURCE_NAME}' (sqlite @ {demo_path})"
                );
            }
        }
        Err(e) => {
            eprintln!("full: federation sidecar install/seed failed: {e} (datasources unavailable)")
        }
    }
}
