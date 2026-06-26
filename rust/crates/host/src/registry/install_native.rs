//! `install_native_from_registry` — the registry × native-tier composition (native-tier + registry
//! scopes): a **signed `tier="native"` artifact** installs through the SAME pull→verify→cache flow a
//! wasm artifact uses, then is **supervised** instead of loaded in-process. This is the proof the two
//! S7 slices compose — one trust model, two runtimes.
//!
//! It composes, it does not re-invent: `pull` does fetch/verify/cache (the signature gate — a
//! tampered native artifact is rejected BEFORE the binary ever touches disk); then the verified
//! binary bytes are written under `install_dir` and `install_native` supervises them (the capability
//! gate + the durable records + the spawn). Both gates hold: signature (in `pull`) and capability
//! (in `install_native`) — installing a native extension bypasses neither.

use std::io::Write;

use lb_ext_loader::Manifest;
use lb_registry::{TrustedKeys, Visibility};

use super::error::RegistryServiceError;
use super::pull::pull;
use super::source::Source;
use crate::boot::Node;
use crate::native::{install_native, Supervised};

/// Install (or roll back to) a signed NATIVE `ext_id`@`version` in workspace `ws` from `source`,
/// supervising the child via `launcher`. Pulls + verifies (offline-served if cached), writes the
/// verified binary under `install_dir`, then runs the native install (persist grant + spawn). The
/// manifest MUST be `tier="native"`; a wasm artifact uses `install_from_registry` instead.
#[allow(clippy::too_many_arguments)]
pub async fn install_native_from_registry<S: Source, L: lb_supervisor::Launcher>(
    node: &Node,
    source: &S,
    launcher: &L,
    caller: &lb_auth::Principal,
    ws: &str,
    ext_id: &str,
    version: &str,
    install_dir: &str,
    trusted: &TrustedKeys,
    admin_approved: &[String],
    ts: u64,
) -> Result<Supervised, RegistryServiceError> {
    // 1. Pull + VERIFY (offline-served if cached). The signature gate runs here, before disk I/O.
    let artifact = pull(
        &node.store,
        source,
        ws,
        ext_id,
        version,
        trusted,
        Visibility::Private,
        ts,
    )
    .await?;

    // 2. Land the verified binary on disk so the supervisor has something to exec. The manifest's
    //    `[native] exec` names the file within install_dir.
    let manifest = Manifest::parse(&artifact.manifest_toml)
        .map_err(|e| RegistryServiceError::Load(crate::load::LoadError::Manifest(e.to_string())))?;
    let exec = manifest
        .native
        .as_ref()
        .map(|n| n.exec.clone())
        .ok_or_else(|| {
            RegistryServiceError::Load(crate::load::LoadError::Manifest(
                "registry native install: manifest is not native".into(),
            ))
        })?;
    write_executable(install_dir, &exec, &artifact.wasm)?;

    // 3. Supervise it through the native install (the capability gate + durable records + spawn).
    let supervised = install_native(
        node,
        launcher,
        caller,
        ws,
        &artifact.manifest_toml,
        install_dir,
        admin_approved,
        ts,
    )
    .await
    .map_err(native_to_registry)?;
    Ok(supervised)
}

/// Write `bytes` as an executable file `dir/name` (creating `dir`). On Unix it is chmod'd `0755` so
/// the supervisor can exec it. A store/IO failure maps to the registry error domain.
fn write_executable(dir: &str, name: &str, bytes: &[u8]) -> Result<(), RegistryServiceError> {
    std::fs::create_dir_all(dir).map_err(io_err)?;
    let path = std::path::Path::new(dir).join(name);
    let mut f = std::fs::File::create(&path).map_err(io_err)?;
    f.write_all(bytes).map_err(io_err)?;
    f.flush().map_err(io_err)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).map_err(io_err)?;
    }
    Ok(())
}

fn io_err(e: std::io::Error) -> RegistryServiceError {
    RegistryServiceError::Load(crate::load::LoadError::Manifest(format!(
        "writing native binary: {e}"
    )))
}

fn native_to_registry(e: crate::native::NativeServiceError) -> RegistryServiceError {
    use crate::native::NativeServiceError as N;
    match e {
        N::Denied => RegistryServiceError::Denied,
        N::Store(s) => RegistryServiceError::Store(s),
        N::Load(l) => RegistryServiceError::Load(l),
        other => RegistryServiceError::Load(crate::load::LoadError::Runtime(other.to_string())),
    }
}
