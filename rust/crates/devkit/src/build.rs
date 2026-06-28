use std::path::Path;

use anyhow::Result;

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
    }
    Ok(BuildReport {
        status: BuildStatus::Done,
    })
}
