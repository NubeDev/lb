//! Read/write Claude Code's `~/.claude/settings.json`.
//!
//! One responsibility: turn a provider's `env` block into the `env` field of that JSON
//! file, preserving every other top-level key Claude Code may have written.
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::{Map, Value};

/// Resolve `~/.claude/settings.json`, honouring `$HOME`.
pub fn path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine $HOME")?;
    Ok(home.join(".claude").join("settings.json"))
}

/// Apply (or replace) the `env` block in settings.json with the given entries. Other
/// top-level keys are preserved. Missing parent dirs and the file itself are created.
pub fn write_env(env: &std::collections::BTreeMap<String, String>) -> Result<PathBuf> {
    let path = path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let mut root: Value = if path.exists() {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if raw.trim().is_empty() {
            Value::Object(Map::new())
        } else {
            serde_json::from_str(&raw)
                .with_context(|| format!("failed to parse JSON in {}", path.display()))?
        }
    } else {
        Value::Object(Map::new())
    };

    let obj = root
        .as_object_mut()
        .context("settings.json root is not a JSON object")?;
    let env_obj = env
        .iter()
        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
        .collect();
    obj.insert("env".into(), Value::Object(env_obj));

    let pretty =
        serde_json::to_string_pretty(&root).context("failed to serialize settings.json")?;
    fs::write(&path, format!("{pretty}\n"))
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

/// Read just the `env` block currently in settings.json (for `status` display).
pub fn read_env() -> Result<Option<Map<String, Value>>> {
    let path = path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(None);
    }
    let root: Value = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(root.get("env").and_then(|v| v.as_object().cloned()))
}
