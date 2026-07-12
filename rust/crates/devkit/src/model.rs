use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Feature;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    Wasm,
    Native,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Wasm => "wasm",
            Tier::Native => "native",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInfo {
    pub tier: Tier,
    pub label: String,
    pub features: Vec<Feature>,
    pub world: String,
}

/// Tolerant deserialization for the `features` field: models frequently pass it as a **stringified
/// JSON array** (`"[\"ui\", \"series-read\"]"`) instead of a real array. This helper accepts BOTH
/// shapes — a real array deserializes normally; a string is parsed as JSON first. Absent ⇒ empty.
fn deserialize_features<'de, D>(deserializer: D) -> Result<Vec<Feature>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Array(Vec<Feature>),
        String(String),
    }

    match Option::<Raw>::deserialize(deserializer)? {
        None => Ok(Vec::new()),
        Some(Raw::Array(v)) => Ok(v),
        Some(Raw::String(s)) => {
            // The model passed a stringified JSON array — parse it.
            let parsed: Vec<Feature> = serde_json::from_str(&s)
                .map_err(|e| Error::custom(format!("features string is not valid JSON: {e}")))?;
            Ok(parsed)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaffoldRequest {
    pub id: String,
    pub tier: Tier,
    #[serde(default, deserialize_with = "deserialize_features")]
    pub features: Vec<Feature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaffoldReport {
    pub path: PathBuf,
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteFileReport {
    /// The canonical absolute path that was written (resolved under the devkit root).
    pub path: PathBuf,
    /// The number of UTF-8 bytes written.
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildRequest {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildReport {
    pub status: BuildStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BuildStatus {
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectReport {
    pub id: String,
    pub tier: Tier,
    pub tools: Vec<String>,
    pub caps: Vec<String>,
    pub built: bool,
    pub toolchain: ToolchainReadiness,
    /// The concrete build outputs found on disk (native/wasm binary, federated `remoteEntry.js`),
    /// each with its current size + mtime. The UI snapshots these before a build and diffs against a
    /// fresh inspect after, so "built" becomes *proof this build wrote a fresh artifact*, not just
    /// "a release dir exists". Empty when nothing has been built yet.
    pub artifacts: Vec<BuildArtifact>,
}

/// One build output on disk. `kind` classifies it (`native-bin` | `wasm` | `remote-entry`); `path`
/// is absolute; `size`/`mtime` come from the same `fs::metadata` facts `host.fs.stat` reports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildArtifact {
    pub kind: String,
    pub path: PathBuf,
    pub size: u64,
    /// RFC3339 UTC modified time, seconds precision — matches `host.fs.stat`'s `mtime` format so the
    /// UI can compare snapshots lexically.
    pub mtime: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolchainReadiness {
    pub cargo: bool,
    pub pnpm: bool,
    pub wasm32_wasip2: bool,
}
