//! `host.fs.list` — one directory level of host filesystem metadata.

use std::fs;

use lb_mcp::ToolError;
use serde::Serialize;
use serde_json::Value;

use super::{path, stat};

pub const HOST_FS_LIST_LIMIT: usize = 1_000;

#[derive(Debug, Clone, Serialize)]
pub struct HostFsList {
    pub path: String,
    pub os: String,
    pub entries: Vec<HostFsEntry>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostFsEntry {
    pub name: String,
    pub kind: String,
    pub size: Option<u64>,
}

pub fn host_fs_list(input: &Value) -> Result<HostFsList, ToolError> {
    let host_path = path::parse_path(input)?;
    let mut entries = Vec::new();
    let mut truncated = false;

    for entry in
        fs::read_dir(&host_path.raw).map_err(|e| ToolError::BadInput(format!("list: {e}")))?
    {
        if entries.len() >= HOST_FS_LIST_LIMIT {
            truncated = true;
            break;
        }
        let entry = entry.map_err(|e| ToolError::BadInput(format!("list entry: {e}")))?;
        let name = entry.file_name().to_string_lossy().to_string();
        let meta = fs::symlink_metadata(entry.path())
            .map_err(|e| ToolError::BadInput(format!("entry metadata: {e}")))?;
        let kind = stat::kind(&meta).to_string();
        let size = stat::size(&meta, &kind);
        entries.push(HostFsEntry { name, kind, size });
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(HostFsList {
        path: host_path.normalized,
        os: host_path.os,
        entries,
        truncated,
    })
}
