//! `rules.eval` — the **flow-facing** rule entry (rules-workflow-convergence scope, slice 1). It is
//! `rules.run` re-shaped for a flow node: it takes the flow **message envelope** in (`payload` +
//! `topic` + any carried fields, plus explicit `params`) and returns `{output, findings, log}` so a
//! run's findings render on the canvas. Same `RuleEngine`, same cage, same per-source `caps::check` —
//! only the argument mapping + result shape differ, so a rule authored in the UI's `rhai`/`rule` node
//! behaves identically to one run via `rules.run`.
//!
//! Two selections, exactly like `rules.run`: an inline `body` (the `rhai` node) or a saved `rule_id`
//! (the `rule` node — run a stored rule by name + params). The optional `timeout_ms` overrides the
//! cage's wall-clock deadline for THIS eval (the per-node knob, slice 2) — a flow author bounds a
//! heavy rule without touching node config. Gated `mcp:rules.eval:call` at the bridge; the node
//! dispatches it under the caller's own authority (`caller ∩ grant`, no widening).

use std::sync::Arc;
use std::time::Duration;

use lb_auth::Principal;
use lb_rules::RuleLimits;
use serde_json::{Map, Value};

use crate::boot::Node;

use super::config::rule_limits;
use super::error::RulesError;
use super::run::{rules_run, RunResult};
use super::seam::RuleModel;

/// The envelope keys that carry flow *routing*, not rule *data* — surfaced to the rule as params
/// like everything else, but named here so the mapping is explicit (D6).
const ENVELOPE_KEYS: &[&str] = &["payload", "topic"];

/// Run a rule for a flow node. `envelope` is the resolved node inputs (`{payload, topic, ...}`); its
/// fields become rule params (so a rule reads `params.payload`), merged UNDER an explicit `params`
/// object if one was supplied (explicit params win). `body` xor `rule_id` selects the rule; a
/// `timeout_ms` (if present) overrides the cage deadline for this eval. Returns the run result — the
/// caller (the `rhai`/`rule` node) projects `output`/`findings` onto the emitted envelope.
#[allow(clippy::too_many_arguments)]
pub async fn rules_eval(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    body: Option<String>,
    rule_id: Option<String>,
    envelope: &Map<String, Value>,
    explicit_params: &Value,
    timeout_ms: Option<u64>,
    model: Arc<dyn RuleModel>,
    now: u64,
) -> Result<RunResult, RulesError> {
    let params = envelope_to_params(envelope, explicit_params);
    let limits = timeout_ms.map(override_timeout);
    rules_run(
        node, principal, ws, body, rule_id, params, model, now, limits,
    )
    .await
}

/// Fold the flow envelope's fields + an explicit `params` object into one rhai param map. Every
/// envelope field (`payload`, `topic`, carried fields) is exposed as a param of the same name;
/// explicit `params` are layered on top (an author-supplied `params.payload` overrides the
/// envelope's) so a `rule` node can pass fixed args alongside the live message.
fn envelope_to_params(envelope: &Map<String, Value>, explicit_params: &Value) -> rhai::Map {
    let mut merged = Map::new();
    for (k, v) in envelope {
        merged.insert(k.clone(), v.clone());
    }
    // The envelope keys are always present (even if null) so a rule can `params.payload` safely.
    for key in ENVELOPE_KEYS {
        merged.entry((*key).to_string()).or_insert(Value::Null);
    }
    if let Some(obj) = explicit_params.as_object() {
        for (k, v) in obj {
            merged.insert(k.clone(), v.clone());
        }
    }
    super::run::params_to_rhai(&Value::Object(merged))
}

/// Build a `RuleLimits` that keeps every node-config governor but replaces the wall-clock deadline
/// with the node's `timeout_ms` (the per-node knob exposing `RuleLimits.timeout`, slice 2).
fn override_timeout(timeout_ms: u64) -> RuleLimits {
    RuleLimits {
        timeout: Duration::from_millis(timeout_ms),
        ..rule_limits()
    }
}
