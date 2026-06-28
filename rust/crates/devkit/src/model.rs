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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaffoldRequest {
    pub id: String,
    pub tier: Tier,
    pub features: Vec<Feature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaffoldReport {
    pub path: PathBuf,
    pub files: Vec<PathBuf>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolchainReadiness {
    pub cargo: bool,
    pub pnpm: bool,
    pub wasm32_wasip2: bool,
}
