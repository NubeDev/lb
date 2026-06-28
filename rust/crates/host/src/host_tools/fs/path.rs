//! Host path normalization for uniform JSON across operating systems.

use std::path::{Path, PathBuf};

use lb_mcp::ToolError;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct HostPath {
    pub raw: PathBuf,
    pub normalized: String,
    pub os: String,
}

pub fn parse_path(input: &Value) -> Result<HostPath, ToolError> {
    let raw = input
        .get("path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| ToolError::BadInput("missing arg: path".into()))?;
    Ok(normalize_path(Path::new(raw)))
}

pub fn normalize_path(path: &Path) -> HostPath {
    let raw = path.to_path_buf();
    let normalized = path.to_string_lossy().replace('\\', "/");
    HostPath {
        raw,
        normalized,
        os: std::env::consts::OS.to_string(),
    }
}
