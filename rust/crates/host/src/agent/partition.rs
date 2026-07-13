//! Partition one turn's proposed calls by the workspace **permission policy** (agent-run Part 2)
//! clamped by the active persona's supervision floor (persona-coding #4): Allow → dispatch now
//! (still capability-checked), Deny → a "denied by policy" result the model sees, Ask → suspend
//! the run for a human decision. Pure — the loop owns the persistence and the suspension.

use super::decision::DENIED_BY_POLICY;
use super::model_access::{CallOutcome, ProposedCall};
use super::personas::{clamp_to_preset, PolicyPreset};
use super::policy::{evaluate, Effect, Policy};

/// One turn's calls, partitioned by effect.
pub(super) struct Partitioned<'c> {
    pub to_run: Vec<ProposedCall>,
    pub denied: Vec<CallOutcome>,
    pub ask: Vec<&'c ProposedCall>,
}

/// Evaluate the ws policy per call (args parsed once for the shallow arg match), then apply the
/// persona preset as a FLOOR clamp — an Ask/Deny floor on a node-mutating tool tightens the
/// evaluated effect unless the ws policy explicitly ruled on that exact tool. `None` preset → the
/// evaluated effect stands.
pub(super) fn partition_by_policy<'c>(
    calls: Vec<&'c ProposedCall>,
    policy: &Policy,
    preset: Option<&PolicyPreset>,
) -> Partitioned<'c> {
    let mut out = Partitioned {
        to_run: Vec::new(),
        denied: Vec::new(),
        ask: Vec::new(),
    };
    for c in calls {
        let args = serde_json::from_str(&c.input).unwrap_or(serde_json::Value::Null);
        let effect = clamp_to_preset(evaluate(policy, &c.name, &args), &c.name, policy, preset);
        match effect {
            Effect::Allow => out.to_run.push(c.clone()),
            Effect::Deny => out.denied.push(CallOutcome {
                id: c.id.clone(),
                name: c.name.clone(),
                input: c.input.clone(),
                ok: None,
                error: Some(DENIED_BY_POLICY.to_string()),
            }),
            Effect::Ask => out.ask.push(c),
        }
    }
    out
}
