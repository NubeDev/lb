//! `case_merge` — fold one case into another, over the authoring capability.
//!
//! Gates on `mcp:case.open:call` (aliased `case.merge → case.open`): merging is the same authority
//! as creating a grouping — it MOVES citations between cases, which is what opening one does.
//! Without the alias the outer gate would demand an `mcp:case.merge:call` that exists in no role
//! bundle, and the verb would be `Denied` for every caller including admins — the shipped-but-
//! unusable trap `tool_gate.rs` documents four times.
//!
//! `skip_human_placed: false` here — a PERSON asked for this merge, and a person may overrule a
//! person. The reactors call the crate verb with `true`; that asymmetry is the whole design.

use std::sync::Arc;

use lb_auth::Principal;
use lb_cases::Case;
use lb_mcp::authorize_tool;

use super::error::CaseSvcError;
use crate::boot::Node;

/// Merge `from_id` into `into_id` in `ws` as `principal`. The losing case closes as `duplicate`.
pub async fn case_merge(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    from_id: &str,
    into_id: &str,
    ts: u64,
) -> Result<Case, CaseSvcError> {
    authorize_tool(principal, ws, "case.open").map_err(|_| CaseSvcError::Denied)?;
    let closed = lb_cases::merge(
        &node.store,
        ws,
        from_id,
        into_id,
        principal.sub(),
        false,
        ts,
    )
    .await?;
    // Re-echo: every member that moved now belongs to `into_id`, and a roster still pointing at the
    // closed case would render a chip for work that no longer exists.
    super::echo::echo_members_of(node, ws, into_id).await;
    Ok(closed)
}
