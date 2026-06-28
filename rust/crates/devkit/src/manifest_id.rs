use anyhow::{anyhow, Context, Result};
use toml::Value;

/// Read only the artifact metadata that must be duplicated outside the manifest for registry lookup.
/// The loader still reparses `manifest_toml` as source of truth, and the digest binds this manifest
/// string to the signed bytes.
pub(crate) fn manifest_id_version(manifest_toml: &str) -> Result<(String, String)> {
    let parsed: Value = toml::from_str(manifest_toml).context("parse extension manifest toml")?;
    let extension = parsed
        .get("extension")
        .and_then(Value::as_table)
        .ok_or_else(|| anyhow!("manifest missing [extension] table"))?;
    let id = string_field(extension, "id")?;
    let version = string_field(extension, "version")?;
    Ok((id, version))
}

fn string_field(table: &toml::Table, key: &str) -> Result<String> {
    table
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow!("manifest missing [extension] {key}"))
}
