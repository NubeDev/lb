//! Run a SAVED rule by id — the host-internal seam shared by the `rules.run` verb and
//! `pack.apply`'s run-on-first-apply.
//!
//! It exists so those two callers cannot drift on the two things that are easy to get subtly wrong:
//! resolving the workspace's model (an unconfigured workspace must get the honest `DisabledModel`,
//! never a fabricated answer) and the `route` default (`true` — a rule run is meant to raise). A
//! second hand-rolled copy of that in the pack module would be exactly the "verb-name folklore"
//! class of bug moving the engine into core is supposed to end.
//!
//! Authority is unchanged: [`rules_run`] gates as it always does, under the caller's principal.

use std::sync::Arc;

use lb_auth::Principal;

use super::error::RulesError;
use super::run::{rules_run, RunResult};
use crate::boot::Node;

/// Run saved rule `id` in `ws` as `principal`, with the workspace's resolved model and the routing
/// default. `now` is the run's logical clock — the caller supplies it (determinism).
pub async fn rules_run_by_id(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    id: &str,
    now: u64,
) -> Result<RunResult, RulesError> {
    let idem = super::idem_prefix(ws, None, Some(id), now);
    let model = super::resolve_rule_model(node, principal, ws, idem).await;
    rules_run(
        node,
        principal,
        ws,
        None,
        Some(id.to_string()),
        rhai::Map::new(),
        model,
        now,
        None,
        true,
    )
    .await
}
