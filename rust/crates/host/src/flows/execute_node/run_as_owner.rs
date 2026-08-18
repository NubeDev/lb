//! Run-as-owner for a HEADLESS rule fire — the run-as-owner slot `ext-store-nodes-scope` §348
//! reserved, built for lb#167's second half.
//!
//! **The problem this solves.** A scheduled rule fires under the fixed system principal
//! `node:reactor` (`reactor_caps()`), whose grant is deliberately narrow: it carries the flow-run
//! surface, the platform store verbs, and (since lb#167) `rules.eval` + the raise/alert verbs. It
//! does NOT carry `mcp:federation.query:call`, so the moment a rule body's first statement is
//! `query("<datasource>", ...)` — the ordinary shape for every BMS/FDD rule in the estate — the
//! collect is denied and every single fire is a `partialFailure`, while `rules.run` from the UI
//! succeeds under the user's own token. That manual-vs-headless asymmetry is lb#167 exactly, one
//! verb deeper than the caps fix addressed.
//!
//! **Why not just widen `reactor_caps()`.** Adding `federation.query` there would hand EVERY
//! scheduled flow on the node blanket read access to EVERY registered datasource, forever, with no
//! relationship to who authored the thing. That is the blanket third-party reach `reactor_caps()`'s
//! own comment rejects for `ext-call`, and the next verb a rule body needs would re-open the same
//! argument. The scope's answer is to stop asking "what may the reactor do" and ask "who is this
//! run acting for".
//!
//! **The rule.** A scheduled rule runs as its OWNER — the subject that saved it with the directive
//! (`SavedRule::scheduled_by`) — and never with more than that owner holds *right now*:
//!
//!   - the owner is an **identity**, not a stored credential. Caps are re-resolved from the live
//!     grant store on every fire (`resolve_caps_live`), so a demoted, revoked, or deleted author's
//!     schedule loses exactly the reach they lost, on the very next fire. Nothing is frozen in.
//!   - the minted principal is **workspace-pinned** to the firing workspace, so an owner with reach
//!     in two workspaces cannot make a ws-A schedule read ws-B.
//!   - the fold is `owner ∪ reactor` ∩ nothing — the reactor's own run-surface caps are kept so the
//!     mechanics of running the flow still work, and the owner's caps supply the data reach. It is
//!     never a widening beyond the owner: the owner already had every cap the rule body uses, since
//!     they could always run it by hand.
//!   - it is **fail-closed**. No `scheduled_by` (a rule saved before this shipped), an unresolvable
//!     owner, or a store error → `None` → the caller keeps the reactor principal and behaves exactly
//!     as it did before. A missing owner degrades to the old denial, never to a widening.
//!
//! Manual runs are untouched: `rules.run`/an interactive flow run already carries the caller's own
//! principal, and this is only consulted when the incoming principal IS the reactor.

use std::sync::Arc;

use lb_auth::Principal;

use crate::authz::resolve_caps_live;
use crate::boot::Node;
use crate::rules::{SavedRule, RULE_TABLE};

/// The fixed system subject the flow reactors mint per tick (`reactor_loop::spawn_flow_reactors`).
/// Matching on it is what distinguishes a HEADLESS fire from a user-driven run: only the former
/// needs an owner substituted, and only the former is denied without one.
const REACTOR_SUB: &str = "node:reactor";

/// The `user:` prefix a principal's `sub` carries but the grant store does NOT.
const USER_PREFIX: &str = "user:";

/// Resolve the principal a `rule` node should execute under.
///
/// Returns `Some(owner_principal)` only for a headless fire of a rule that records an owner whose
/// caps still resolve; `None` everywhere else, meaning "keep the principal you were given". See the
/// module docs for why every failure path is `None` rather than an error.
pub async fn owner_principal(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    rule_id: &str,
) -> Option<Principal> {
    // Only a headless fire substitutes. A user-driven run already carries the right authority, and
    // swapping it would be a privilege CHANGE (in either direction) on a path nobody asked about.
    if principal.sub() != REACTOR_SUB {
        return None;
    }
    let owner = scheduled_by(node, ws, rule_id).await?;
    // Grants are stored under the BARE user name; the resolver re-wraps it as `Subject::User`. The
    // recorded owner is a principal `sub` (`user:test`), so it MUST be stripped before resolving —
    // handing the prefixed form straight to the resolver silently returns zero caps, which reads as
    // "revoked owner" and degrades to the reactor principal. That is the same prefix bug lb#176 hit
    // on the reminder fire path; it is nearly invisible because every failure mode here is a
    // deliberate `None`.
    let bare = owner.strip_prefix(USER_PREFIX).unwrap_or(&owner);
    let mut caps = resolve_caps_live(&node.store, ws, bare).await.ok()?;
    if caps.is_empty() {
        // A subject that resolves to nothing is a revoked/deleted owner. Keep the reactor principal
        // so the failure reads as the ordinary deny it is, rather than an empty-cap mystery.
        return None;
    }
    // Keep the reactor's own run-surface caps: the node still has to drive the flow machinery it was
    // invoked from (run-store writes, step output), which is the reactor's job and not the owner's.
    caps.extend(super::super::reactor_caps());
    caps.sort();
    caps.dedup();
    Some(Principal::routed(owner, ws.to_string(), caps))
}

/// Read `scheduled_by` off the saved rule. Any miss (absent rule, tombstoned, unparseable record, no
/// recorded owner) is `None` — the fail-closed path.
async fn scheduled_by(node: &Arc<Node>, ws: &str, rule_id: &str) -> Option<String> {
    let val = lb_store::read(&node.store, ws, RULE_TABLE, rule_id)
        .await
        .ok()??;
    let rule: SavedRule = serde_json::from_value(val).ok()?;
    if rule.deleted {
        return None;
    }
    rule.scheduled_by.filter(|s| !s.is_empty())
}
