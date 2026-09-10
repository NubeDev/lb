//! `case_split` — pull members out into a NEW case, over the authoring capability.
//!
//! Gates on `mcp:case.open:call` (aliased `case.split → case.open`) — see [`super::merge`] for why
//! both ride the grouping-authoring grant rather than minting two caps no bundle carries.
//!
//! The new case is `grouping: human` and every moved member is stamped `human_placed`, so no
//! reactor may re-fold what a person deliberately separated. That is enforced in the crate verb.

use std::sync::Arc;

use lb_auth::Principal;
use lb_cases::Case;
use lb_mcp::authorize_tool;

use super::error::CaseSvcError;
use crate::boot::Node;

/// Split `insight_ids` out of `from_id` into a new case titled `title`, as `principal`.
pub async fn case_split(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    from_id: &str,
    insight_ids: &[String],
    title: &str,
    ts: u64,
) -> Result<Case, CaseSvcError> {
    authorize_tool(principal, ws, "case.open").map_err(|_| CaseSvcError::Denied)?;
    let new_case = lb_cases::split(
        &node.store,
        ws,
        from_id,
        insight_ids,
        title,
        principal.sub(),
        ts,
    )
    .await?;
    super::echo::echo_members_of(node, ws, &new_case.id).await;
    Ok(new_case)
}
