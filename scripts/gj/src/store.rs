//! Read/write the `jobs.yaml` store and resolve its path.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::model::Store;

/// Resolve the jobs file: `--file` → `$GJ_FILE` → `~/.config/gj/jobs.yaml`.
/// A user-level default keeps the path stable for the absolute `ExecStart` in systemd units.
pub fn path(flag: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = flag {
        return Ok(path);
    }
    if let Ok(path) = std::env::var("GJ_FILE") {
        return Ok(PathBuf::from(path));
    }
    let dir = dirs::config_dir().context("no config dir for gj")?.join("gj");
    Ok(dir.join("jobs.yaml"))
}

pub fn load(path: &Path) -> Result<Store> {
    if !path.exists() {
        return Ok(Store::default());
    }
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_yml::from_str(&raw).with_context(|| format!("parse {}", path.display()))
}

pub fn save(path: &Path, store: &Store) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create {}", parent.display()))?;
    }
    let raw = serde_yml::to_string(store).context("serialize jobs")?;
    fs::write(path, raw).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}
