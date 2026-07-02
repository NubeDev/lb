//! `control-engine.schema` — read the type catalogue (the add-node palette: every
//! extension's manifest + its component types). Maps onto `ControlEngine::get_schema`.
//!
//! No args beyond the envelope `appliance`. The `Vec<ExtensionManifest>` result is
//! returned VERBATIM under `{ "manifests": [...] }` (each `ExtensionManifest`
//! derives `Serialize`; no bigint-hostile bare u64 crosses the JS boundary here).

use rubix_ce::ControlEngine;
use serde_json::{json, Value};

/// Run `control-engine.schema`. Calls `get_schema` and returns the manifest list
/// verbatim.
pub async fn run(engine: &dyn ControlEngine) -> Result<Value, String> {
    let manifests = engine.get_schema().await.map_err(|e| e.to_string())?;
    Ok(json!({ "manifests": manifests }))
}
