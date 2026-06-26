//! The single chokepoint: the two-gate capability check (auth-caps scope, README §3.5/§3.6).
//!
//! Gate 1 — **workspace isolation** (hard wall): `principal.ws == request.ws`, else
//! [`Denied::Workspace`]. No capability can override this. Gate 2 — **capability**: some held
//! cap pattern-matches the request, else [`Denied::Capability`]. Every surface (store, bus,
//! mcp, secret) routes through here before touching the resource.

use lb_auth::Principal;
use thiserror::Error;

use crate::grammar::matches;
use crate::request::Request;

/// The result of a check: allowed, or denied with the gate that failed.
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Allowed,
    Denied(Denied),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Denied {
    /// Gate 1: the principal's workspace does not match the requested resource's workspace.
    #[error("workspace isolation: principal not in target workspace")]
    Workspace,
    /// Gate 2: no held capability grants this request.
    #[error("capability: no grant for this request")]
    Capability,
}

/// Run the gates in order. This is the only authorization entry point in the host.
///
/// For a **delegated** principal (the agent acting for a caller, agent scope) there is a third
/// gate: the request must ALSO match the delegation `constraint` (the caller's caps). That makes
/// the effective grant `agent ∩ caller` — a delegated actor can never do something *either* side
/// forbids, so an agent cannot widen its access. An ordinary principal has no constraint and is
/// bounded by gate 2 alone.
pub fn check(principal: &Principal, req: &Request) -> Decision {
    // Gate 1: isolation first — the hard wall, before any capability is consulted.
    if principal.ws() != req.ws {
        return Decision::Denied(Denied::Workspace);
    }
    // Gate 2: the actor's own capabilities must grant the request.
    if !matches(principal.caps(), req) {
        return Decision::Denied(Denied::Capability);
    }
    // Gate 2b (delegation): if on-behalf-of, the caller's caps must ALSO grant it (intersection).
    // No constraint → an ordinary actor, already allowed by gate 2.
    if let Some(constraint) = principal.constraint() {
        if !matches(constraint, req) {
            return Decision::Denied(Denied::Capability);
        }
    }
    Decision::Allowed
}
