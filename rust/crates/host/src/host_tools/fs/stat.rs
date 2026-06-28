//! `host.fs.stat` — metadata for one host path.

use std::fs::{self, File, OpenOptions};
use std::time::UNIX_EPOCH;

use lb_mcp::ToolError;
use serde::Serialize;
use serde_json::Value;

use super::path;

#[derive(Debug, Clone, Serialize)]
pub struct HostFsStat {
    pub path: String,
    pub os: String,
    pub exists: bool,
    pub kind: String,
    pub size: Option<u64>,
    pub mtime: Option<String>,
    pub readable: bool,
    pub writable: bool,
}

pub fn host_fs_stat(input: &Value) -> Result<HostFsStat, ToolError> {
    let host_path = path::parse_path(input)?;
    let meta = match fs::symlink_metadata(&host_path.raw) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(HostFsStat {
                path: host_path.normalized,
                os: host_path.os,
                exists: false,
                kind: "missing".to_string(),
                size: None,
                mtime: None,
                readable: false,
                writable: false,
            });
        }
        Err(e) => return Err(ToolError::BadInput(format!("metadata: {e}"))),
    };

    let kind = kind(&meta);
    Ok(HostFsStat {
        path: host_path.normalized,
        os: host_path.os,
        exists: true,
        kind: kind.to_string(),
        size: size(&meta, kind),
        mtime: mtime(&meta),
        readable: readable(&host_path.raw, kind),
        writable: writable(&host_path.raw, kind, &meta),
    })
}

pub fn kind(meta: &fs::Metadata) -> &'static str {
    let file_type = meta.file_type();
    if file_type.is_symlink() {
        "symlink"
    } else if file_type.is_dir() {
        "dir"
    } else if file_type.is_file() {
        "file"
    } else {
        "other"
    }
}

pub fn size(meta: &fs::Metadata, kind: &str) -> Option<u64> {
    (kind == "file").then_some(meta.len())
}

pub fn mtime(meta: &fs::Metadata) -> Option<String> {
    let modified = meta.modified().ok()?;
    let duration = modified.duration_since(UNIX_EPOCH).ok()?;
    let dt = chrono::DateTime::<chrono::Utc>::from(UNIX_EPOCH + duration);
    Some(dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

fn readable(path: &std::path::Path, kind: &str) -> bool {
    match kind {
        "dir" => fs::read_dir(path).is_ok(),
        _ => File::open(path).is_ok(),
    }
}

fn writable(path: &std::path::Path, kind: &str, meta: &fs::Metadata) -> bool {
    match kind {
        "file" | "symlink" => OpenOptions::new().write(true).open(path).is_ok(),
        _ => !meta.permissions().readonly(),
    }
}
