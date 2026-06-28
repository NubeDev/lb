//! `history.list` and `history.compensations` — the read side of the undo stack, over the gate.
//!
//! Gated by `mcp:history.list:call` (and `undo.any` for another actor's stack). Reads are state, not
//! motion — returned as a list. Authorization only; the read stays in `lb_undo::{list,compensations}`.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;
use lb_undo::{compensations, list, HistoryItem};

use super::error::UndoSvcError;

/// List `actor`'s `surface` stack in `ws`, newest-first, as `principal`.
pub async fn history_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
    actor: &str,
    surface: &str,
) -> Result<Vec<HistoryItem>, UndoSvcError> {
    authorize_tool(principal, ws, "history.list").map_err(|_| UndoSvcError::Denied)?;
    if actor != principal.sub() {
        authorize_tool(principal, ws, "undo.any").map_err(|_| UndoSvcError::Denied)?;
    }
    Ok(list(store, ws, actor, surface).await?)
}

/// The compensating tool a non-undoable step `seq` offers, if any. Gated by the same verb cap.
pub async fn history_compensations(
    store: &Store,
    principal: &Principal,
    ws: &str,
    seq: u64,
) -> Result<Option<String>, UndoSvcError> {
    authorize_tool(principal, ws, "history.list").map_err(|_| UndoSvcError::Denied)?;
    Ok(compensations(store, ws, seq).await?)
}
