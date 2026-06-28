//! `emit`/`alert`/`log` — collect findings + log lines per run. **Ported from rubix-cube's
//! `verbs/emit.rs`**. `emit` records a finding; `alert` records a finding marked `alert:true` (the
//! host routes those to the inbox + outbox after the run — rules-engine-scope resolves rubix-cube's
//! stage-03 TODO). `log` records a line. The collectors are drained into the [`crate::runtime::RuleRun`]
//! after evaluation.

use std::sync::{Arc, Mutex};

use rhai::{Engine, Map};

use crate::grid::dynamic_to_json;
use crate::runtime::{Finding, LogLine};

/// The shared per-run collectors. Behind a `Mutex` so the (Send+Sync) verb closures can append.
#[derive(Default)]
pub struct Collectors {
    pub findings: Mutex<Vec<Finding>>,
    pub log: Mutex<Vec<LogLine>>,
}

impl Collectors {
    pub fn drain_findings(&self) -> Vec<Finding> {
        std::mem::take(&mut self.findings.lock().unwrap())
    }
    pub fn drain_log(&self) -> Vec<LogLine> {
        std::mem::take(&mut self.log.lock().unwrap())
    }
}

pub fn register(engine: &mut Engine, collectors: Arc<Collectors>) {
    {
        let c = collectors.clone();
        engine.register_fn("emit", move |map: Map| {
            c.findings.lock().unwrap().push(finding_from(&map, false));
        });
    }
    {
        let c = collectors.clone();
        engine.register_fn("alert", move |map: Map| {
            c.findings.lock().unwrap().push(finding_from(&map, true));
        });
    }
    {
        let c = collectors.clone();
        engine.register_fn("log", move |msg: &str| {
            c.log.lock().unwrap().push(LogLine {
                level: "info".to_string(),
                message: msg.to_string(),
            });
        });
    }
}

/// Build a `Finding` from an emitted map: lift `level` for filtering; the whole map rides as `data`.
fn finding_from(map: &Map, is_alert: bool) -> Finding {
    let level = map
        .get("level")
        .and_then(|v| v.clone().into_string().ok())
        .unwrap_or_else(|| "info".to_string());

    let mut data = serde_json::Map::new();
    for (k, v) in map.iter() {
        data.insert(k.to_string(), dynamic_to_json(v));
    }
    if is_alert {
        data.insert("alert".to_string(), serde_json::Value::Bool(true));
    }
    Finding {
        level,
        data: serde_json::Value::Object(data),
    }
}
