//! Parse `extension.toml` — the §13 forever contract (extensions scope). TOML, declared
//! tools (so the host can register + authorize without instantiating), requested caps (a
//! request, never a grant), and the WIT world major (checked against the host's SDK).

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ManifestError {
    #[error("manifest is not valid TOML: {0}")]
    Toml(String),
    #[error("extension declares WIT world '{0}' incompatible with this host")]
    WorldMismatch(String),
    #[error("unknown runtime tier '{0}' (expected wasm | native)")]
    UnknownTier(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Tool {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub id: String,
    pub version: String,
    pub tier: String,
    pub world: String,
    pub placement: String,
    /// Capabilities the extension REQUESTS — intersected with admin approval by `grant`.
    pub requested_caps: Vec<String>,
    pub tools: Vec<Tool>,
    pub visibility: Visibility,
}

// Raw TOML shape, mapped to the flat `Manifest` after validation.
#[derive(Deserialize)]
struct Raw {
    extension: RawExt,
    runtime: RawRuntime,
    #[serde(default)]
    capabilities: RawCaps,
    #[serde(default)]
    tools: Vec<Tool>,
    visibility: RawVisibility,
}
#[derive(Deserialize)]
struct RawExt {
    id: String,
    version: String,
}
#[derive(Deserialize)]
struct RawRuntime {
    tier: String,
    world: String,
    placement: String,
}
#[derive(Deserialize, Default)]
struct RawCaps {
    #[serde(default)]
    request: Vec<String>,
}
#[derive(Deserialize)]
struct RawVisibility {
    class: Visibility,
}

impl Manifest {
    /// Parse + validate a manifest's TOML text. Rejects an unknown tier and a WIT world whose
    /// major does not match this host's SDK (the load-time ABI check, crate-layout scope).
    pub fn parse(text: &str) -> Result<Self, ManifestError> {
        let raw: Raw = toml::from_str(text).map_err(|e| ManifestError::Toml(e.to_string()))?;

        if raw.runtime.tier != "wasm" && raw.runtime.tier != "native" {
            return Err(ManifestError::UnknownTier(raw.runtime.tier));
        }
        if !lb_sdk::world_major_matches(&raw.runtime.world) {
            return Err(ManifestError::WorldMismatch(raw.runtime.world));
        }

        Ok(Manifest {
            id: raw.extension.id,
            version: raw.extension.version,
            tier: raw.runtime.tier,
            world: raw.runtime.world,
            placement: raw.runtime.placement,
            requested_caps: raw.capabilities.request,
            tools: raw.tools,
            visibility: raw.visibility.class,
        })
    }
}
