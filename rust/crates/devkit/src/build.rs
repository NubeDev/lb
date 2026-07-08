use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::{inspect_extension, BuildReport, BuildStatus, Tier, Toolchain};

pub fn build_extension(
    path: &Path,
    toolchain: &dyn Toolchain,
    log: &mut dyn FnMut(String),
) -> Result<BuildReport> {
    let inspected = inspect_extension(path)?;
    let cargo_args: &[&str] = match inspected.tier {
        Tier::Wasm => &["build", "--target", "wasm32-wasip2", "--release"],
        Tier::Native => &["build", "--release"],
    };
    log(format!("==> cargo {}", cargo_args.join(" ")));
    toolchain.run(path, "cargo", cargo_args, log)?;
    if path.join("ui").is_dir() {
        log("==> pnpm install".into());
        toolchain.run(&path.join("ui"), "pnpm", &["install"], log)?;
        log("==> pnpm vite build".into());
        toolchain.run(&path.join("ui"), "pnpm", &["exec", "vite", "build"], log)?;
        // Deploy the built UI dist to the gateway's ext-ui serve dir so the federated page is
        // reachable immediately after build (no separate copy step). The dir is the same env var
        // the gateway reads (`LB_EXT_UI_DIR`, default `extensions-ui`). Best-effort: a failure
        // (unset env, permission) is logged but does not fail the build — the wasm half is still
        // valid; the UI can be deployed separately.
        deploy_ui_dist(path, &inspected.id, log);
    }
    Ok(BuildReport {
        status: BuildStatus::Done,
    })
}

/// Copy `<ext>/ui/dist/*` → `<LB_EXT_UI_DIR>/<id>/` so the gateway serves the federated page at
/// `/extensions/<id>/ui/remoteEntry.js`. Best-effort — the env may be unset in a pure CLI build
/// (e.g. `lb-pack`), and a deployment failure is not a build failure.
fn deploy_ui_dist(ext_path: &Path, id: &str, log: &mut dyn FnMut(String)) {
    let src = ext_path.join("ui/dist");
    if !src.is_dir() {
        return;
    }
    // Same default as the gateway (`extensions-ui` beside the cwd); the env override is for
    // non-standard layouts. Both resolve to the same dir the gateway serves from.
    let dest_root = std::env::var_os("LB_EXT_UI_DIR")
        .unwrap_or_else(|| std::ffi::OsString::from("extensions-ui"));
    let dest = Path::new(&dest_root).join(id);
    if let Err(e) = fs::create_dir_all(&dest) {
        log(format!("  (ext-ui deploy: create_dir_all failed: {e})"));
        return;
    }
    if let Err(e) = copy_dir(&src, &dest) {
        log(format!("  (ext-ui deploy: copy failed: {e})"));
        return;
    }
    log(format!(
        "  -> deployed UI bundle to {} (served at /extensions/{id}/ui/)",
        dest.display()
    ));
}

fn copy_dir(src: &Path, dest: &Path) -> Result<()> {
    for entry in fs::read_dir(src).context("read dir")? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        let ft = entry.file_type()?;
        if ft.is_dir() {
            fs::create_dir_all(&to)?;
            copy_dir(&from, &to)?;
        } else {
            fs::copy(&from, &to).context("copy")?;
        }
    }
    Ok(())
}
