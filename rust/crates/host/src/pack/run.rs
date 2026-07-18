//! Running a pack's rule once on first apply — the same path `rules.run` takes, model resolution
//! included, so a pack-run rule and a hand-run rule behave identically.
//!
//! `route: true` (the default `rules.run` uses): a pack's first apply is the moment its rules are
//! MEANT to raise — the demo oracle is precisely "blank node → one apply → a real insight raises".
//! Suppressing the fan-out here would make the pack apply quietly and the operator see nothing.

use std::sync::Arc;

use lb_auth::Principal;

use crate::boot::Node;

/// Run saved rule `id` once, under the caller's authority. Errors are returned for the caller to
/// record as a warning — a rule that saves but does not run is not a failed object.
pub async fn run_saved_rule(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    id: &str,
    ts: u64,
) -> Result<(), String> {
    crate::rules::rules_run_by_id(node, principal, ws, id, ts)
        .await
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
}
