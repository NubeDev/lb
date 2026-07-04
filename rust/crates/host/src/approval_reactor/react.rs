//! `react_to_approval_releases` — the **generic approval-release reactor**: a durable scan that, the
//! moment a `needs:approval` item resolves, releases its held effect on `Approved` (`held → pending`,
//! now deliverable by the existing relay) or discards it on `Rejected` (`held → discarded`, never
//! sent). A `Deferred` item is left held (inert in v1). This is the domain-free sibling of the
//! coding-workflow's `resolve_approval` reactor (rules-approvals scope, Open-questions option b): it
//! keeps that path untouched and keys only on `(resolution, held effect id)`, so it treats the effect
//! as opaque data (rule 10) — no extension/effect semantics.
//!
//! Altitude — a durable scan, not a LIVE-query reactor (like `relay_outbox`/`react_to_approvals`): the
//! scan is the source of truth, so a reactor that restarts re-reads `approved`/`rejected` and never
//! misses a resolution. One pass at logical time `now` releases/discards every owed effect; call it
//! again and it is a no-op.
//!
//! Idempotency — the release/discard are **guarded transitions** in `lb_outbox` (they act only on a
//! currently-`Held` effect). So a replay (a second tick, or a deferred-then-approved item) finds the
//! effect already `Pending`/`Discarded` and does nothing: an effect is released **exactly once** (the
//! relay never double-delivers) and a reject after an approve cannot claw a released effect back. An
//! approved resolution whose item has **no** held effect (a plain `inbox.resolve`, or a coding-job
//! approval the workflow reactor owns) is simply a no-op here — `release` returns `false` for an
//! absent effect. The two reactors are safe to run over the same resolutions.
//!
//! Authorization — the reactor runs under a host **service principal** (the node acting on its own
//! durable state), and the release/discard are raw store transitions gated by the *resolution
//! existing*, NOT by re-checking a user cap. This is the load-bearing trust boundary: the **request**
//! was caller-gated (`request_approval` ran the caller's caps); the **release** is a system transition
//! — so a released effect can never exceed what was staged, and no user token can force a release
//! (there is no user verb for it). The hard wall holds: a ws-B pass selects ws-B's namespace for the
//! `approved`/`rejected` scans and the effect transitions, so it can physically only touch ws-B
//! effects (mandatory isolation §7).

use lb_inbox::{approved, rejected};
use lb_outbox::{discard, release};

use super::id::held_effect_id;
use crate::boot::Node;

/// The outcome of one reactor pass over a workspace: how many held effects were released (on approval)
/// and how many discarded (on rejection). Resolutions with no held effect are not counted (no-op).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ApprovalReleasePass {
    pub released: usize,
    pub discarded: usize,
}

/// Run one release pass over workspace `ws`: release the held effect of every `Approved` item and
/// discard the held effect of every `Rejected` item. Idempotent (the guarded transitions make a
/// replay a no-op). Returns the pass tally. `ws` selects the namespace, so this only ever touches
/// `ws`'s effects.
pub async fn react_to_approval_releases(
    node: &Node,
    ws: &str,
) -> Result<ApprovalReleasePass, lb_store::StoreError> {
    let mut pass = ApprovalReleasePass::default();

    for resolution in approved(&node.store, ws).await? {
        let effect_id = held_effect_id(&resolution.item_id);
        if release(&node.store, ws, &effect_id).await? {
            pass.released += 1;
        }
    }

    for resolution in rejected(&node.store, ws).await? {
        let effect_id = held_effect_id(&resolution.item_id);
        if discard(&node.store, ws, &effect_id).await? {
            pass.discarded += 1;
        }
    }

    Ok(pass)
}
