//! `redo` — re-apply the newest redoable step on an actor's stack, over the capability gate.
//! Same three gates as [`undo`](super::undo) (`mcp:redo:call`, no-escalation on the step's tool,
//! `undo.any` for another actor). Authorization only; the conditional re-apply stays in
//! `lb_undo::apply_redo`.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;
use lb_undo::{apply_redo, peek_redo, JournalEntry};

use super::error::UndoSvcError;

/// Redo the newest redoable step on `actor`'s `surface` stack in `ws`, as `principal`.
pub async fn redo(
    store: &Store,
    principal: &Principal,
    ws: &str,
    actor: &str,
    surface: &str,
) -> Result<JournalEntry, UndoSvcError> {
    authorize_tool(principal, ws, "redo").map_err(|_| UndoSvcError::Denied)?;
    if actor != principal.sub() {
        authorize_tool(principal, ws, "undo.any").map_err(|_| UndoSvcError::Denied)?;
    }
    if let Some(entry) = peek_redo(store, ws, actor, surface).await? {
        authorize_tool(principal, ws, &entry.tool).map_err(|_| UndoSvcError::Denied)?;
    }
    Ok(apply_redo(store, ws, actor, surface).await?)
}
