//! `systemd --user` timer + service unit management. Best-effort: callers treat a
//! missing/unavailable `systemctl` as a warning, not a hard failure, so the job record
//! is still saved and the units can be installed later with `gj install <id>`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

use anyhow::{bail, Context, Result};

use crate::model::Job;

/// `~/.config/systemd/user` — where `--user` units live.
fn unit_dir() -> Result<PathBuf> {
    Ok(dirs::config_dir()
        .context("no config dir")?
        .join("systemd")
        .join("user"))
}

fn service_name(id: &str) -> String {
    format!("gj-{id}.service")
}

fn timer_name(id: &str) -> String {
    format!("gj-{id}.timer")
}

/// Write the `.service` + `.timer` units for `job`, reload, and (if enabled) start the timer.
/// `store_path` is baked into the service's `ExecStart` so the timer runs the same job file.
pub fn install(job: &Job, store_path: &Path) -> Result<()> {
    let dir = unit_dir()?;
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;

    let bin = env::current_exe().context("locate gj binary")?;
    let store_abs = fs::canonicalize(store_path).unwrap_or_else(|_| store_path.to_path_buf());

    let service = format!(
        "[Unit]\nDescription=gj auto-commit {id} ({repo})\n\n\
         [Service]\nType=oneshot\nExecStart={bin} --file {file} run {id}\n",
        id = job.id,
        repo = job.repo,
        bin = bin.display(),
        file = store_abs.display(),
    );
    let timer = format!(
        "[Unit]\nDescription=gj auto-commit timer {id}\n\n\
         [Timer]\n{clause}Persistent=true\n\n\
         [Install]\nWantedBy=timers.target\n",
        id = job.id,
        clause = job.timer_clause()?,
    );

    fs::write(dir.join(service_name(&job.id)), service)?;
    fs::write(dir.join(timer_name(&job.id)), timer)?;

    systemctl(&["daemon-reload"])?;
    if job.enabled {
        systemctl(&["enable", "--now", &timer_name(&job.id)])?;
    }
    Ok(())
}

/// Start (`enable --now`) or stop (`disable --now`) an installed timer.
pub fn set_enabled(id: &str, enabled: bool, store_path: &Path, job: &Job) -> Result<()> {
    // Make sure units exist before toggling (a job added while systemd was unavailable).
    if !unit_dir()?.join(timer_name(id)).exists() {
        return install(job, store_path);
    }
    let verb = if enabled { "enable" } else { "disable" };
    systemctl(&[verb, "--now", &timer_name(id)])
}

/// Stop, disable, and delete the units for `id`.
pub fn remove(id: &str) -> Result<()> {
    // Best-effort stop/disable; ignore "not loaded" errors.
    let _ = systemctl(&["disable", "--now", &timer_name(id)]);
    let dir = unit_dir()?;
    for name in [service_name(id), timer_name(id)] {
        let path = dir.join(name);
        if path.exists() {
            fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
        }
    }
    let _ = systemctl(&["daemon-reload"]);
    Ok(())
}

/// Run `systemctl --user <args>`, mapping a missing binary / no user session to a clear error.
fn systemctl(args: &[&str]) -> Result<()> {
    let out = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output();
    let out = match out {
        Ok(out) => out,
        Err(err) => bail!("systemctl --user not available ({err})"),
    };
    if !out.status.success() {
        bail!(
            "systemctl --user {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}
