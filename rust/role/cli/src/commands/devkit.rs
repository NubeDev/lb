//! `lb devkit sign <name-or-path> [--out <file>]` — fold `lb-pack` into the CLI (operator-cli scope).
//! Produces the signed `Artifact` JSON the registry's `verify_artifact` accepts, over the SAME
//! `lb-devkit` library `lb-pack` uses (byte-identical signing). The header still prints so the operator
//! sees the context; the body is the artifact JSON (to stdout — a pipe can feed it to `lb ext publish`).
//!
//! `sign` is a client-only, offline operation (no transport) — but it still prints the session header
//! for a consistent surface, using whichever session the run resolved.

use crate::error::{CliError, CliResult};
use crate::header::Header;
use crate::sign::sign_extension;

use super::Printed;

/// Sign `name_or_path` into an artifact. If `out` is given, write the JSON there and print a one-line
/// confirmation; otherwise print the artifact JSON to stdout (pipeable into `lb ext publish -`).
pub fn sign(header: &Header, name_or_path: &str, out: Option<&str>) -> CliResult<Printed> {
    let artifact = sign_extension(name_or_path)?;
    let json = serde_json::to_string_pretty(&artifact)
        .map_err(|e| CliError::Other(format!("serialize artifact: {e}")))?;
    match out {
        Some(path) => {
            if let Some(parent) = std::path::Path::new(path)
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent)
                    .map_err(|e| CliError::Other(format!("create {parent:?}: {e}")))?;
            }
            std::fs::write(path, &json)
                .map_err(|e| CliError::Other(format!("write artifact {path}: {e}")))?;
            Ok(Printed::new(
                header,
                format!(
                    "signed {} v{} → {path}  (publisher: {})",
                    artifact.ext_id, artifact.version, artifact.publisher_key_id
                ),
            ))
        }
        None => Ok(Printed::new(header, json)),
    }
}
