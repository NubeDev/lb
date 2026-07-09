//! `host.fs.list` — one directory level of host filesystem metadata.
//!
//! Optional filters (all applied per-entry, before the limit):
//! - `name`: case-insensitive substring the entry name must contain.
//! - `extensions`: array of file extensions (with or without a leading `.`, e.g.
//!   `"txt"` or `".txt"`); an entry matches if its extension equals one of them
//!   case-insensitively. Directories/symlinks never match an extension filter.
//! - `include_hidden`: when false (the default) dot-prefixed entries are skipped.
//!   Hidden-ness is by leading-dot name, uniform across linux/darwin/windows.

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

struct Filter {
    name: Option<String>,
    extensions: Vec<String>,
    include_hidden: bool,
}

fn parse_filter(input: &Value) -> Filter {
    let name = input
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());
    let extensions = input
        .get("extensions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.trim().trim_start_matches('.').to_lowercase())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let include_hidden = input
        .get("include_hidden")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    Filter {
        name,
        extensions,
        include_hidden,
    }
}

fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

fn ext_matches(name: &str, kind: &str, extensions: &[String]) -> bool {
    if extensions.is_empty() {
        return true;
    }
    // Only real files carry a meaningful extension for this filter.
    if kind != "file" {
        return false;
    }
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => {
            extensions.iter().any(|e| e == &ext.to_lowercase())
        }
        _ => false,
    }
}

fn keep(name: &str, kind: &str, filter: &Filter) -> bool {
    if !filter.include_hidden && is_hidden(name) {
        return false;
    }
    if let Some(needle) = &filter.name {
        if !name.to_lowercase().contains(needle.as_str()) {
            return false;
        }
    }
    ext_matches(name, kind, &filter.extensions)
}

pub fn host_fs_list(input: &Value) -> Result<HostFsList, ToolError> {
    let host_path = path::parse_path(input)?;
    let filter = parse_filter(input);
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
        if !keep(&name, &kind, &filter) {
            continue;
        }
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
