//! Sign a built extension into the `Artifact` JSON the gateway verifies (operator-cli scope: the
//! `lb-pack` retirement). This folds over the EXISTING `lb-devkit` library — the same
//! `sign_artifact` (digest + Ed25519) `lb-pack` and the host publish path use — so a `lb devkit sign`
//! artifact is byte-identical to `lb-pack`'s and verifies by construction (there is no second crypto
//! idiom, rule 9). This module is only path resolution + file IO around that one call.
//!
//! It resolves `<name-or-path>` under the devkit root exactly like the gateway's dev publish shortcut
//! (`resolve_under_root` + `inspect_extension` + the tier's built-binary path), so `lb devkit sign
//! hello-v2` and the browser's "publish" reach the same bytes with the same key.

use std::path::{Path, PathBuf};

use lb_devkit::{
    default_devkit_root, inspect_extension, load_or_create_key, resolve_under_root, sign_artifact,
    InspectReport, Tier,
};
use lb_registry::Artifact;

use crate::error::{CliError, CliResult};

/// The default publisher key id — matches `lb-pack` and the gateway's dev publish shortcut, so the
/// same `dev-publisher.key` produces artifacts every path trusts alike.
pub const DEFAULT_KEY_ID: &str = "dev-publisher";

/// Sign the extension at `name_or_path` (resolved under the devkit root) into a signed `Artifact`.
/// Reads the manifest + the built binary for the manifest's tier, then signs with the dev publisher
/// key (created on first use). The exact bytes the registry's `verify_artifact` accepts.
pub fn sign_extension(name_or_path: &str) -> CliResult<Artifact> {
    let root = default_devkit_root();
    let path = resolve_under_root(&root, name_or_path)
        .map_err(|e| CliError::BadInput(format!("resolve {name_or_path}: {e}")))?;
    let manifest_path = path.join("extension.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .map_err(|e| CliError::BadInput(format!("read {}: {e}", manifest_path.display())))?;
    let inspect = inspect_extension(&path)
        .map_err(|e| CliError::BadInput(format!("inspect {}: {e}", path.display())))?;
    let bytes_path = built_binary_path(&path, &inspect);
    let bytes = std::fs::read(&bytes_path).map_err(|e| {
        CliError::BadInput(format!(
            "read build output {} (did you build it? `build.sh`): {e}",
            bytes_path.display()
        ))
    })?;
    let loaded = load_or_create_key(&key_path())
        .map_err(|e| CliError::Other(format!("load/create publisher key: {e}")))?;
    sign_artifact(manifest, bytes, DEFAULT_KEY_ID, &loaded.signing_key)
        .map_err(|e| CliError::Other(format!("sign artifact: {e}")))
}

/// The dev publisher key path — under the devkit root's `keys/`, the same file the gateway's dev
/// publish shortcut reads. `load_or_create_key` mints it on first use.
pub fn key_path() -> PathBuf {
    default_devkit_root().join("keys").join("dev-publisher.key")
}

/// The built-binary path for the manifest's tier (mirrors the gateway's `built_binary_path`): a wasm
/// component under `target/wasm32-wasip2/release/{id}_ext.wasm`, or a native binary under
/// `target/release/{id}`.
fn built_binary_path(path: &Path, inspect: &InspectReport) -> PathBuf {
    match inspect.tier {
        Tier::Wasm => path
            .join("target/wasm32-wasip2/release")
            .join(format!("{}_ext.wasm", inspect.id.replace('-', "_"))),
        Tier::Native => path.join("target/release").join(&inspect.id),
    }
}
