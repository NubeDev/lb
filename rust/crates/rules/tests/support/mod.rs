//! Test support — a recording data seam + a scripted AI seam. These are NOT fakes of node behavior:
//! they stand in for the HOST's seam impl (the sanctioned trait boundary), feeding seeded rows into
//! the REAL engine path so the engine's composition/governors/fence/budget are exercised for real.
//! The real store/caps/federation path is tested in the host integration tests (no mocks there).
#![allow(dead_code, unused_imports)] // each test binary uses a different subset of these helpers

use std::collections::BTreeMap;
use std::sync::Mutex;

use lb_rules::seam::{AiSeam, DataSeam, SchemaColumn, SourceKind};
use lb_rules::{AiCompletion, GridJson};

/// A data seam that resolves a fixed source map, records every collected query, and returns seeded
/// rows for the matching source. `Federation` sources resolve to the federation kind.
pub struct RecordingData {
    pub platform_sources: Vec<String>,
    pub federation_sources: Vec<String>,
    pub rows: GridJson,
    pub collected: Mutex<Vec<String>>,
    pub schemas: BTreeMap<String, Vec<SchemaColumn>>,
}

impl RecordingData {
    pub fn platform(sources: &[&str], rows: GridJson) -> Self {
        Self {
            platform_sources: sources.iter().map(|s| s.to_string()).collect(),
            federation_sources: Vec::new(),
            rows,
            collected: Mutex::new(Vec::new()),
            schemas: BTreeMap::new(),
        }
    }

    pub fn last_query(&self) -> Option<String> {
        self.collected.lock().unwrap().last().cloned()
    }
}

impl DataSeam for RecordingData {
    fn resolve(&self, source: &str) -> Result<(SourceKind, String), String> {
        if self.platform_sources.iter().any(|s| s == source) {
            Ok((SourceKind::Platform, source.to_string()))
        } else if self.federation_sources.iter().any(|s| s == source) {
            Ok((SourceKind::Federation, source.to_string()))
        } else {
            Err(format!("source not allowed: {source}"))
        }
    }

    fn collect(&self, _kind: SourceKind, _source: &str, query: &str) -> Result<GridJson, String> {
        self.collected.lock().unwrap().push(query.to_string());
        // size()/count reductions get a count row; a scalar reduction (`... AS v`) gets a v row;
        // otherwise the seeded rows.
        if query.contains("count()") || query.contains("GROUP ALL") {
            return Ok(GridJson {
                columns: vec!["v".into()],
                rows: vec![serde_json::json!({ "v": self.rows.rows.len() as i64 })],
            });
        }
        if query.contains(" AS v FROM") {
            return Ok(GridJson {
                columns: vec!["v".into()],
                rows: vec![serde_json::json!({ "v": 7.0 })],
            });
        }
        Ok(self.rows.clone())
    }

    fn schemas(&self) -> Result<BTreeMap<String, Vec<SchemaColumn>>, String> {
        Ok(self.schemas.clone())
    }
}

/// An AI seam scripted with fixed completions + a fixed proposed-SQL. The "malicious LLM" variant
/// proposes a query the fence must re-validate.
pub struct ScriptedAi {
    pub completion: String,
    pub tokens: u32,
    pub proposed_sql: String,
}

impl AiSeam for ScriptedAi {
    fn complete(&self, _prompt: &str) -> Result<AiCompletion, String> {
        Ok(AiCompletion {
            text: self.completion.clone(),
            tokens: self.tokens,
        })
    }

    fn propose_sql(
        &self,
        _question: &str,
        _schemas: &BTreeMap<String, Vec<SchemaColumn>>,
    ) -> Result<String, String> {
        Ok(self.proposed_sql.clone())
    }
}
