//! [`ContainerToolchain`] — runs `cargo`/`pnpm` inside the pinned `docker/build/` image instead of
//! as a bare child of the node process (devkit-container-build-scope.md). Same [`Toolchain`] trait
//! as [`crate::ProcessToolchain`], so `build_extension` and the job/log/publish contract are
//! unchanged — only the executor moves.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

use anyhow::{bail, Context, Result};

use crate::Toolchain;

/// Config for a [`ContainerToolchain`] — the pinned image tag, the cargo/pnpm cache volume, and an
/// optional build-scoped git credential (never baked into the image, mounted per-build only).
pub struct ContainerConfig {
    /// The pinned build image, e.g. `"lazybones-build:latest"` (`docker/build/`).
    pub image: String,
    /// Named volume for the shared cargo/pnpm registry cache (build scratch, not source — see
    /// scope doc "Cache correctness across builds").
    pub cache_volume: String,
    /// A build-scoped git token (e.g. for `NubeIO/ce-client-rust`), injected as a credential
    /// helper inside the container — never interpolated into a URL, never logged.
    pub git_token: Option<String>,
}

pub struct ContainerToolchain {
    pub config: ContainerConfig,
}

impl Toolchain for ContainerToolchain {
    fn run(
        &self,
        cwd: &Path,
        program: &str,
        args: &[&str],
        log: &mut dyn FnMut(String),
    ) -> Result<()> {
        let cwd = cwd
            .canonicalize()
            .with_context(|| format!("canonicalize {}", cwd.display()))?;
        // A generated extension's Cargo.toml has `path = "../../crates/..."` dependencies that
        // escape its own subtree up into the `rust/` workspace (devkit templates are not workspace
        // members, by design). Mounting only the extension dir breaks those; mount the `rust/`
        // workspace root instead and set the working dir to the extension's path inside it, so the
        // container sees exactly the same tree layout `ProcessToolchain` sees on the host.
        let workspace_root = find_workspace_root(&cwd).unwrap_or_else(|| cwd.clone());
        let rel_cwd = cwd.strip_prefix(&workspace_root).unwrap_or(Path::new("."));
        let container_cwd = Path::new("/work").join(rel_cwd);
        let mount = format!("{}:/work", workspace_root.display());
        let cache_mount = format!("{}:/usr/local/cargo/registry", self.config.cache_volume);

        let mut docker_args: Vec<String> = vec![
            "run".into(),
            "--rm".into(),
            // Run as the host uid/gid, not the image's default (root) — otherwise every file
            // `cargo`/`pnpm` writes under the mounted extension tree comes back root-owned and
            // the host node process (an unprivileged user) can neither read the built artifact
            // nor clean up a failed build's `target/`.
            "-u".into(),
            format!("{}:{}", host_uid(), host_gid()),
            // The image's default $HOME (root's) isn't writable by an arbitrary host uid; point
            // both at a scratch dir cargo/pnpm can write their own dotfiles/config into.
            "-e".into(),
            "HOME=/tmp".into(),
            // The mounted `rust/` workspace carries a HOST-specific `.cargo/config.toml` that pins
            // the x86_64 linker + CC/AR/RANLIB at a personal zig toolchain under
            // `/home/user/.local/bin` (that box has no system C compiler). Those paths don't exist
            // in this image — and it has a real GCC toolchain — so the build dies with
            // `linker /home/user/.local/bin/zigcc not found` the moment any build-script/proc-macro
            // crate links for the host target (both wasm and native builds compile those on the
            // host triple). cargo config's `[env]` doesn't override a process env var unless it sets
            // `force = true` (this one doesn't), so exporting the genuine tools here wins for
            // CC/AR/RANLIB. The `[target.*].linker` key can't be beaten by env alone — that's
            // overridden with a `--config` flag on the cargo call below.
            "-e".into(),
            "CC=x86_64-linux-gnu-gcc".into(),
            "-e".into(),
            "AR=x86_64-linux-gnu-ar".into(),
            "-e".into(),
            "RANLIB=x86_64-linux-gnu-ranlib".into(),
            "-e".into(),
            "CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc".into(),
            "-e".into(),
            "AR_x86_64_unknown_linux_gnu=x86_64-linux-gnu-ar".into(),
            "-e".into(),
            "RANLIB_x86_64_unknown_linux_gnu=x86_64-linux-gnu-ranlib".into(),
            "-v".into(),
            mount,
            "-v".into(),
            cache_mount,
            "-w".into(),
            container_cwd.display().to_string(),
        ];

        // The token is passed as an env var into the container only — never woven into a URL
        // (git echoes tokenized URLs back on failure) and never written to the streamed log.
        // The container's git credential helper (baked into the image) reads it from this var.
        if let Some(token) = &self.config.git_token {
            docker_args.push("-e".into());
            docker_args.push(format!("LB_BUILD_GIT_TOKEN={token}"));
        }

        docker_args.push(self.config.image.clone());
        docker_args.push(program.into());
        // The host `.cargo/config.toml` pins `[target.x86_64-unknown-linux-gnu].linker` at the
        // personal zig shim, and a config linker CANNOT be overridden by an env var — only by a
        // higher-precedence `--config` flag. Repoint it at the image's real GCC so host-target
        // build-scripts/proc-macros (compiled for both wasm and native extension builds) link.
        // Inserted right after the cargo subcommand (`build`), before the caller's args, so it
        // applies to `cargo build ...` without disturbing pnpm/other programs.
        if program == "cargo" {
            docker_args.push(
                r#"--config=target.x86_64-unknown-linux-gnu.linker="x86_64-linux-gnu-gcc""#.into(),
            );
        }
        docker_args.extend(args.iter().map(|a| a.to_string()));

        // All sanctioned container execution for devkit builds goes through this function; callers
        // get log lines, never a handle to the child `docker` process. Interleaves stdout/stderr
        // like `ProcessToolchain::run` (scope doc "Log ordering").
        let mut child = Command::new("docker")
            .args(&docker_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| "spawn docker run for devkit container build".to_string())?;

        let (tx, rx) = mpsc::channel::<String>();
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        let tx_out = tx.clone();
        let out_thread = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let _ = tx_out.send(line.unwrap_or_else(|e| format!("stdout read error: {e}")));
            }
        });
        let err_thread = thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                let _ = tx.send(line.unwrap_or_else(|e| format!("stderr read error: {e}")));
            }
        });
        for line in rx {
            log(line);
        }
        let _ = out_thread.join();
        let _ = err_thread.join();

        let status = child.wait().with_context(|| "wait for docker run")?;
        if !status.success() {
            bail!("docker run ({program}) exited with {status}");
        }
        Ok(())
    }

    fn ready(&self, _program: &str) -> bool {
        // Readiness for a container executor means "the runtime and the pinned image are usable",
        // not "the host has cargo/pnpm on PATH" — the scope's "container runtime unavailable" line
        // must be legible instead of a cryptic host spawn error.
        Command::new("docker")
            .args(["image", "inspect", &self.config.image])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn wasm_target_ready(&self) -> bool {
        // wasm32-wasip2 is baked into the pinned image (docker/build/Dockerfile); if the image is
        // present at all, the target is present.
        self.ready("cargo")
    }
}

/// The host's uid, via the `id -u` the container also has (coreutils, baked into every Debian
/// base image). Avoids a libc dependency for a one-shot lookup that only runs once per build.
fn host_uid() -> String {
    id_output("-u")
}

fn host_gid() -> String {
    id_output("-g")
}

fn id_output(flag: &str) -> String {
    Command::new("id")
        .arg(flag)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "0".to_string())
}

/// Walk up from `dir` to the nearest ancestor whose `Cargo.toml` declares a real `[workspace]`
/// with `members` — the `rust/` root. A generated extension's own `Cargo.toml` also carries a bare
/// `[workspace]` (an empty table, opting it OUT of the outer workspace per the devkit template) —
/// skip that one and keep walking, or this would stop at the extension dir itself and the mount
/// would still miss the `../../crates/...` path deps it needs.
fn find_workspace_root(dir: &Path) -> Option<std::path::PathBuf> {
    let mut candidate = dir;
    loop {
        let manifest = candidate.join("Cargo.toml");
        if manifest.is_file() {
            if let Ok(contents) = std::fs::read_to_string(&manifest) {
                if contents.contains("[workspace]") && contents.contains("members") {
                    return Some(candidate.to_path_buf());
                }
            }
        }
        candidate = candidate.parent()?;
    }
}
