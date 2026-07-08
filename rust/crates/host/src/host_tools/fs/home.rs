//! `host.fs.home` — the node's home directory, as a stable absolute anchor a filesystem picker can
//! start browsing from. A caller that needs a node-local path (e.g. the sqlite datasource DB-file
//! picker) starts here rather than at `/`: it is the natural place a user's data lives and lists
//! cleanly, while `host.fs.list` still lets them walk up/anywhere the verb allows. Read-only fact;
//! path is normalized to forward slashes like every other `host.fs.*` result.

use lb_mcp::ToolError;
use serde::Serialize;

use super::path::normalize_path;

#[derive(Debug, Clone, Serialize)]
pub struct HostFsHome {
    pub path: String,
    pub os: String,
}

pub fn host_fs_home() -> Result<HostFsHome, ToolError> {
    let home = dirs::home_dir()
        .ok_or_else(|| ToolError::BadInput("home: no home directory for this node".into()))?;
    let normalized = normalize_path(&home);
    Ok(HostFsHome {
        path: normalized.normalized,
        os: normalized.os,
    })
}
