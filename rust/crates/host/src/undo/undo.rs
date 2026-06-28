//! `undo` — reverse the newest undoable step on an actor's stack, over the capability gate.
//!
//! Three gates, in order (`docs/scope/undo/undo-scope.md` "Capabilities"):
//!   1. `mcp:undo:call` — the verb itself (workspace-first §7).
//!   2. **No escalation:** the caller must hold `mcp:<step.tool>:call` — you cannot reach a mutation
//!      via undo that you couldn't perform directly.
//!   3. **`undo.any`:** acting on *another* actor's stack requires `mcp:undo.any:call`; by default
//!      you undo your own (`principal.sub()`).
//!
//! The actual reversal (the conditional, stale-refusing restore) stays in `lb_undo::apply_undo`;
//! this layer is authorization only (one verb per file, FILE-LAYOUT §3).

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;
use lb_undo::{apply_undo, peek_undo, JournalEntry};

use super::error::UndoSvcError;

/// Undo the newest undoable step on `actor`'s `surface` stack in `ws`, as `principal`. Returns the
/// reversed step. `actor`/`surface` default to the caller's own stack at the call site.
pub async fn undo(
    store: &Store,
    principal: &Principal,
    ws: &str,
    actor: &str,
    surface: &str,
) -> Result<JournalEntry, UndoSvcError> {
    // Gate 1: the verb.
    authorize_tool(principal, ws, "undo").map_err(|_| UndoSvcError::Denied)?;
    // Gate 3: another actor's stack needs undo.any.
    if actor != principal.sub() {
        authorize_tool(principal, ws, "undo.any").map_err(|_| UndoSvcError::Denied)?;
    }
    // Gate 2: no escalation — must hold the original tool's cap. Peek first (no apply).
    if let Some(entry) = peek_undo(store, ws, actor, surface).await? {
        authorize_tool(principal, ws, &entry.tool).map_err(|_| UndoSvcError::Denied)?;
    }
    Ok(apply_undo(store, ws, actor, surface).await?)
}
