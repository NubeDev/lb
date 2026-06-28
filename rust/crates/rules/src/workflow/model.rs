//! The DAG data model + validation — `Chain`/`Step`/`needs`/`with`/`RetrySpec`/`FailurePolicy`/
//! `Trigger` + `validate`/`indegrees`/`dependents`. **Lifted verbatim from rubix-cube's
//! `workflow/model.rs`** (MIT/Apache-2.0), re-keyed `project_id` → `workspace` and `Workflow` → `Chain`.
//! Pure DAG math, no dependency to change. The size cap comes from config.

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

pub type StepId = String;

/// Extra attempts after the first, with a backoff. Maps onto `lb-jobs` retry when a step is a job.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrySpec {
    pub max: u32,
    #[serde(default)]
    pub backoff_ms: u64,
}

/// One DAG node: a saved rule + its upstream deps + input bindings + optional retry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub id: StepId,
    /// The saved rule name this step runs (`rule:{ws}:{rule}`).
    pub rule: String,
    #[serde(default)]
    pub needs: Vec<StepId>,
    /// Input bindings: literal | `${steps.x.output}` | `${steps.x.findings}` | `${params.y}`.
    #[serde(default)]
    pub with: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub retry: Option<RetrySpec>,
}

/// What happens when a step fails after its retries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FailurePolicy {
    /// Prune the failed step's transitive subtree; run = PartialFailure.
    #[default]
    Halt,
    /// Release dependents with the failed output resolved to `null`.
    Continue,
}

/// How a chain run is started.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Trigger {
    /// Via the `chains.run` verb only.
    #[default]
    Manual,
    /// Claimed by the S6 durable-scan reactor when due (window-claim = multi-node safe).
    Cron { expr: String },
    /// A `bus.watch` subscription on a Zenoh subject starts a run on a matching message.
    Event { topic: String },
}

/// A chain — a DAG of steps over saved rules, workspace-scoped.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chain {
    pub workspace: String,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub trigger: Trigger,
    #[serde(default)]
    pub params: serde_json::Map<String, serde_json::Value>,
    pub steps: Vec<Step>,
    #[serde(default)]
    pub failure_policy: FailurePolicy,
}

/// DAG validation faults.
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum DagError {
    #[error("chain has no steps")]
    Empty,
    #[error("too many steps: {0} > {1}")]
    TooManySteps(usize, usize),
    #[error("duplicate step id: {0}")]
    DuplicateStep(StepId),
    #[error("step {0} depends on itself")]
    SelfDependency(StepId),
    #[error("step {0} depends on unknown step {1}")]
    UnknownDependency(StepId, StepId),
    #[error("chain has a cycle")]
    Cycle,
}

impl Chain {
    /// In-degree (number of `needs`) per step.
    pub fn indegrees(&self) -> HashMap<StepId, usize> {
        self.steps
            .iter()
            .map(|s| (s.id.clone(), s.needs.len()))
            .collect()
    }

    /// Reverse edges: step → the steps that depend on it.
    pub fn dependents(&self) -> HashMap<StepId, Vec<StepId>> {
        let mut map: HashMap<StepId, Vec<StepId>> = self
            .steps
            .iter()
            .map(|s| (s.id.clone(), Vec::new()))
            .collect();
        for step in &self.steps {
            for dep in &step.needs {
                if let Some(v) = map.get_mut(dep) {
                    v.push(step.id.clone());
                }
            }
        }
        map
    }

    /// Validate the DAG up front: non-empty, within `max_steps`, unique ids, deps resolve, no
    /// self-edge, acyclic (Kahn). Rejected before any step runs.
    pub fn validate(&self, max_steps: usize) -> Result<(), DagError> {
        if self.steps.is_empty() {
            return Err(DagError::Empty);
        }
        if self.steps.len() > max_steps {
            return Err(DagError::TooManySteps(self.steps.len(), max_steps));
        }
        let mut ids: HashSet<&str> = HashSet::new();
        for step in &self.steps {
            if !ids.insert(step.id.as_str()) {
                return Err(DagError::DuplicateStep(step.id.clone()));
            }
        }
        for step in &self.steps {
            for dep in &step.needs {
                if dep == &step.id {
                    return Err(DagError::SelfDependency(step.id.clone()));
                }
                if !ids.contains(dep.as_str()) {
                    return Err(DagError::UnknownDependency(step.id.clone(), dep.clone()));
                }
            }
        }
        // Kahn: peel in-degree-0 nodes; a remainder means a cycle.
        let mut indeg = self.indegrees();
        let dependents = self.dependents();
        let mut queue: VecDeque<StepId> = indeg
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(k, _)| k.clone())
            .collect();
        let mut processed = 0usize;
        while let Some(id) = queue.pop_front() {
            processed += 1;
            for dep in &dependents[&id] {
                let d = indeg.get_mut(dep).expect("dependent exists");
                *d -= 1;
                if *d == 0 {
                    queue.push_back(dep.clone());
                }
            }
        }
        if processed != self.steps.len() {
            return Err(DagError::Cycle);
        }
        Ok(())
    }

    /// The in-degree-0 frontier (the steps to enqueue at start).
    pub fn frontier(&self) -> Vec<StepId> {
        let mut f: Vec<StepId> = self
            .indegrees()
            .into_iter()
            .filter(|(_, d)| *d == 0)
            .map(|(id, _)| id)
            .collect();
        f.sort();
        f
    }
}
