//! `lb ext publish <name-or-path | artifact.json>` — the `make publish-ext` retirement (operator-cli
//! scope). Two accepted inputs, so the whole curl+jq flow collapses to one command:
//!   - a **signed artifact JSON file** (the output of `lb devkit sign … --out`) → published verbatim;
//!   - an **extension name/dir** → signed on the fly (via `lb devkit sign`'s path) then published.
//! Both end at the transport's [`ExtPublish::publish`], which POSTs `/extensions` (remote) or calls
//! `lb_host::ext_publish` (local). The gateway verifies against ITS trusted keys (remote), so an
//! operator cannot self-trust onto another node; local trusts the operator's own dev key.

use lb_registry::Artifact;

use crate::error::{CliError, CliResult};
use crate::header::Header;
use crate::sign::sign_extension;
use crate::transport::{ExtPublish, PublishOutcome};

use super::Printed;

/// Publish the extension named by `target`. If `target` is a path to a signed artifact JSON, publish
/// it as-is; otherwise treat it as an extension name/dir, sign it, and publish. The header is printed;
/// the body is a one-line outcome (a publish returns no data — a `204`).
pub async fn publish(
    transport: &impl ExtPublish,
    header: &Header,
    target: &str,
) -> CliResult<Printed> {
    let artifact = load_or_sign(target)?;
    let (ext_id, version) = (artifact.ext_id.clone(), artifact.version.clone());
    match transport.publish(artifact).await? {
        PublishOutcome::Published => Ok(Printed::new(
            header,
            format!("published {ext_id} v{version} — verified, installed, loaded live"),
        )),
    }
}

/// Resolve `target` to a signed artifact: parse it as an artifact JSON file if it is one, else sign
/// the named/pathed extension. A `.json` that fails to parse as an `Artifact` is a clean bad-input
/// error (not a silent fall-through to signing a non-existent dir).
fn load_or_sign(target: &str) -> CliResult<Artifact> {
    let looks_like_json = target.ends_with(".json") || std::path::Path::new(target).is_file();
    if looks_like_json {
        let text = std::fs::read_to_string(target)
            .map_err(|e| CliError::BadInput(format!("read artifact {target}: {e}")))?;
        return serde_json::from_str::<Artifact>(&text).map_err(|e| {
            CliError::BadInput(format!(
                "{target} is not a signed artifact JSON (did you mean an extension dir?): {e}"
            ))
        });
    }
    sign_extension(target)
}
