//! The per-table store-mutation gate. `store.write`/`store.delete` are host-native MCP tools; the
//! outer `mcp:store.<verb>:call` gate runs at the dispatcher. Here we run the **store-surface**
//! capability — `store:<table>:write` — through the shared `caps::check` chokepoint (workspace-first
//! §3.6, then capability §3.5). A denial is opaque [`StoreMutateError::Denied`].
//!
//! This is the load-bearing scope for the generic write path: the grant names the TABLE, so a holder
//! of `store:<table>:write` can mutate that table and nothing else — the same grammar the api-key roles
//! (`store:*:write`) and the flows reactor (`store:flow:write`) already use. Mirrors
//! `rules_save`'s `authorize_store_write` (`Request::new(ws, Surface::Store, table, Action::Write)`),
//! generalized over the table.

use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};

use super::error::StoreMutateError;

/// Authorize a mutation of `table` in `ws`. `Ok(())` only if gate 1 (workspace) and
/// `store:<table>:write` both pass. Used by both `store.write` and `store.delete` (a delete is a
/// table mutation gated under the same `write` action — see the module doc).
pub fn authorize_store_mutate(
    principal: &Principal,
    ws: &str,
    table: &str,
) -> Result<(), StoreMutateError> {
    let req = Request::new(ws, Surface::Store, table, Action::Write);
    match check(principal, &req) {
        Decision::Allowed => Ok(()),
        Decision::Denied(_) => Err(StoreMutateError::Denied),
    }
}
