//! Binding resolution + result aggregation — `RunContext`/`Outcome`/`StepRecord`/`ChainResult`/
//! `ChainStatus`/`StepResult`. **Lifted verbatim from rubix-cube's `workflow/context.rs`**
//! (MIT/Apache-2.0). Whole-value `${...}` references only — a binding is exactly one reference or a
//! literal; an embedded `${` is rejected (no templating mini-language). `params` +
//! `steps.<id>.output|findings` are the resolvable namespaces; a failed/skipped upstream resolves to
//! `null` (under Continue).

use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value;

use crate::runtime::{Finding, RuleOutput};

use super::model::StepId;

/// A step's terminal outcome.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// Completed: the output + the findings it emitted.
    Ok(RuleOutput, Vec<Finding>),
    /// Failed after retries (the error message).
    Err(String),
    /// Never ran (upstream failure under Halt).
    Skipped,
}

/// What a step recorded.
#[derive(Debug, Clone)]
pub struct StepRecord {
    pub outcome: Outcome,
    pub attempts: u32,
    pub ms: u64,
}

/// The accumulating run context — params + recorded step outcomes, for binding resolution + the result.
pub struct RunContext {
    params: serde_json::Map<String, Value>,
    records: HashMap<StepId, StepRecord>,
    order: Vec<StepId>,
}

impl RunContext {
    pub fn new(params: serde_json::Map<String, Value>) -> Self {
        Self {
            params,
            records: HashMap::new(),
            order: Vec::new(),
        }
    }

    pub fn record(&mut self, id: StepId, rec: StepRecord) {
        if !self.records.contains_key(&id) {
            self.order.push(id.clone());
        }
        self.records.insert(id, rec);
    }

    pub fn has(&self, id: &str) -> bool {
        self.records.contains_key(id)
    }

    /// Resolve a step's `with` bindings into a rhai map against params + recorded outputs.
    pub fn resolve_bindings(
        &self,
        with: &serde_json::Map<String, Value>,
    ) -> Result<rhai::Map, String> {
        let mut out = rhai::Map::new();
        for (key, value) in with {
            let resolved = self.resolve_value(value)?;
            out.insert(key.as_str().into(), crate::grid::json_to_dynamic(&resolved));
        }
        Ok(out)
    }

    /// Resolve a single binding value: a whole-string `${...}` reference, else a literal.
    pub fn resolve_value(&self, value: &Value) -> Result<Value, String> {
        let Value::String(s) = value else {
            return Ok(value.clone());
        };
        let Some(reference) = parse_reference(s) else {
            return Ok(value.clone());
        };
        Ok(self.lookup(reference))
    }

    fn lookup(&self, reference: Reference<'_>) -> Value {
        match reference {
            Reference::Param(name) => self.params.get(name).cloned().unwrap_or(Value::Null),
            Reference::StepOutput(id) => match self.records.get(id).map(|r| &r.outcome) {
                Some(Outcome::Ok(output, _)) => output_to_json(output),
                _ => Value::Null,
            },
            Reference::StepFindings(id) => match self.records.get(id).map(|r| &r.outcome) {
                Some(Outcome::Ok(_, findings)) => {
                    serde_json::to_value(findings).unwrap_or(Value::Null)
                }
                _ => Value::Null,
            },
        }
    }

    /// Collapse the recorded outcomes into a `ChainResult`.
    pub fn to_result(&self) -> ChainResult {
        let mut steps = Vec::new();
        let mut any_failed = false;
        let mut any_ok = false;
        for id in &self.order {
            let rec = &self.records[id];
            let (status, output, findings, error) = match &rec.outcome {
                Outcome::Ok(out, f) => {
                    any_ok = true;
                    ("ok", Some(output_to_json(out)), f.clone(), None)
                }
                Outcome::Err(e) => {
                    any_failed = true;
                    ("failed", None, Vec::new(), Some(e.clone()))
                }
                Outcome::Skipped => ("skipped", None, Vec::new(), None),
            };
            steps.push(StepResult {
                id: id.clone(),
                status: status.to_string(),
                ms: rec.ms,
                attempts: rec.attempts,
                output,
                findings,
                error,
            });
        }
        let status = if any_failed && any_ok {
            ChainStatus::PartialFailure
        } else if any_failed {
            ChainStatus::Failed
        } else {
            ChainStatus::Success
        };
        ChainResult { status, steps }
    }
}

enum Reference<'a> {
    Param(&'a str),
    StepOutput(&'a str),
    StepFindings(&'a str),
}

/// Parse a whole-string `${...}` reference. Rejects embedded `${`/`}` (only whole references resolve).
fn parse_reference(s: &str) -> Option<Reference<'_>> {
    let inner = s.strip_prefix("${")?.strip_suffix('}')?;
    if inner.contains("${") || inner.contains('}') {
        return None;
    }
    let inner = inner.trim();
    if let Some(name) = inner.strip_prefix("params.") {
        return Some(Reference::Param(name));
    }
    let rest = inner.strip_prefix("steps.")?;
    let (id, field) = rest.rsplit_once('.')?;
    match field {
        "output" => Some(Reference::StepOutput(id)),
        "findings" => Some(Reference::StepFindings(id)),
        _ => None,
    }
}

/// Reduce a `RuleOutput` to the JSON a downstream `${steps.x.output}` sees.
fn output_to_json(output: &RuleOutput) -> Value {
    match output {
        RuleOutput::Scalar(v) => v.clone(),
        RuleOutput::Grid(g) => serde_json::json!({ "columns": g.columns, "rows": g.rows }),
        RuleOutput::Findings | RuleOutput::Nothing => Value::Null,
    }
}

/// The terminal status of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChainStatus {
    Success,
    PartialFailure,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepResult {
    pub id: StepId,
    pub status: String,
    pub ms: u64,
    pub attempts: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<Finding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainResult {
    pub status: ChainStatus,
    pub steps: Vec<StepResult>,
}
