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
    /// A `tier="native"` manifest must carry a `[native]` block naming the `exec` to spawn — the
    /// supervisor has nothing to launch otherwise (native-tier scope). A wasm manifest must NOT.
    #[error("native tier requires a [native] block with exec; wasm tier must omit it")]
    NativeSpec,
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

/// The `[native]` block — present iff `tier="native"` (native-tier scope, the extensions-scope
/// deferred "Native (`tier="native"`) manifest fields (exec, supervision, socket) — S7"). It is the
/// recipe the host turns into a `lb_supervisor::Spec`: which binary to spawn, its args, the platform
/// target the binary is built for (a native binary is NOT portable like a `.wasm`, platform-targets
/// scope), and the restart policy. Health/grace/backoff timings stay host-defaults this slice.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct Native {
    /// The executable the supervisor spawns. Resolved by the host against the install dir.
    pub exec: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// The target triple the binary is built for (platform-targets scope). Empty = host/unspecified.
    #[serde(default)]
    pub target: String,
    /// `"on-crash"` (default) | `"never"` — the crash-restart policy (operator restart is separate).
    #[serde(default)]
    pub restart: String,
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
    /// The native supervision recipe — `Some` iff `tier="native"` (validated at parse). `None` for a
    /// wasm extension (it has no child process).
    pub native: Option<Native>,
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
    #[serde(default)]
    native: Option<Native>,
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

        // The `[native]` block is required for and exclusive to the native tier: the supervisor must
        // know what to spawn, and a wasm extension has no child (native-tier scope).
        let is_native = raw.runtime.tier == "native";
        let native = match (is_native, raw.native) {
            (true, Some(n)) if !n.exec.is_empty() => Some(n),
            (true, _) => return Err(ManifestError::NativeSpec),
            (false, Some(_)) => return Err(ManifestError::NativeSpec),
            (false, None) => None,
        };

        Ok(Manifest {
            id: raw.extension.id,
            version: raw.extension.version,
            tier: raw.runtime.tier,
            world: raw.runtime.world,
            placement: raw.runtime.placement,
            requested_caps: raw.capabilities.request,
            tools: raw.tools,
            visibility: raw.visibility.class,
            native,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NATIVE_TOML: &str = r#"
[extension]
id = "echo-sidecar"
version = "0.1.0"

[runtime]
tier = "native"
world = "lazybones:ext/extension@0.1.0"
placement = "either"

[native]
exec = "echo-sidecar"
args = ["--serve"]
restart = "on-crash"

[[tools]]
name = "echo"

[visibility]
class = "private"
"#;

    fn with_runtime(tier: &str, native_block: &str) -> String {
        format!(
            r#"
[extension]
id = "x"
version = "0.1.0"
[runtime]
tier = "{tier}"
world = "lazybones:ext/extension@0.1.0"
placement = "either"
{native_block}
[visibility]
class = "private"
"#
        )
    }

    #[test]
    fn parses_native_block() {
        let m = Manifest::parse(NATIVE_TOML).expect("native manifest parses");
        assert_eq!(m.tier, "native");
        let n = m.native.expect("native tier carries a [native] block");
        assert_eq!(n.exec, "echo-sidecar");
        assert_eq!(n.args, vec!["--serve".to_string()]);
        assert_eq!(n.restart, "on-crash");
    }

    #[test]
    fn native_tier_without_exec_is_rejected() {
        // tier=native but no [native] block → NativeSpec (the supervisor has nothing to spawn).
        let toml = with_runtime("native", "");
        assert_eq!(Manifest::parse(&toml), Err(ManifestError::NativeSpec));
    }

    #[test]
    fn wasm_tier_with_native_block_is_rejected() {
        // A wasm extension must not carry supervision fields (it has no child).
        let toml = with_runtime("wasm", "[native]\nexec = \"oops\"");
        assert_eq!(Manifest::parse(&toml), Err(ManifestError::NativeSpec));
    }

    #[test]
    fn wasm_tier_omits_native() {
        let toml = with_runtime("wasm", "");
        let m = Manifest::parse(&toml).expect("wasm manifest parses");
        assert!(m.native.is_none());
    }
}
