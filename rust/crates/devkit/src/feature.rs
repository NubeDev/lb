use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Feature {
    Ui,
    SeriesRead,
    Ingest,
    Kv,
    /// Query external datasources via the federation sidecar (`federation.query`,
    /// `federation.schema`, `datasource.list`). The "connect to the server" feature — an
    /// extension that reads from a postgres/sqlite/SurrealDB source needs this.
    Datasources,
}

impl Feature {
    pub fn all() -> Vec<Self> {
        vec![
            Self::Ui,
            Self::SeriesRead,
            Self::Ingest,
            Self::Kv,
            Self::Datasources,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ui => "ui",
            Self::SeriesRead => "series-read",
            Self::Ingest => "ingest",
            Self::Kv => "kv",
            Self::Datasources => "datasources",
        }
    }
}

pub fn feature_caps(features: &[Feature]) -> Vec<String> {
    let mut caps = Vec::new();
    if features.contains(&Feature::SeriesRead) {
        caps.extend([
            "mcp:series.find:call".to_string(),
            "mcp:series.latest:call".to_string(),
            "mcp:series.read:call".to_string(),
        ]);
    }
    if features.contains(&Feature::Ingest) {
        caps.push("mcp:ingest.write:call".to_string());
    }
    if features.contains(&Feature::Kv) {
        caps.push("mcp:template.get:call".to_string());
        caps.push("mcp:template.save:call".to_string());
    }
    if features.contains(&Feature::Datasources) {
        caps.extend([
            "mcp:federation.query:call".to_string(),
            "mcp:federation.schema:call".to_string(),
            "mcp:datasource.list:call".to_string(),
        ]);
    }
    caps.sort();
    caps.dedup();
    caps
}
