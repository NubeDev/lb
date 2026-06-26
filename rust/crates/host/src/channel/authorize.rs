//! The channel authorization gate — the single chokepoint every channel verb passes through
//! before touching the bus or the store (capability-first, §3.5).
//!
//! It is the *same* `caps::check` the MCP and store surfaces use: gate 1 workspace isolation
//! (so a principal in workspace B is refused a channel in workspace A before any capability is
//! read), gate 2 the `bus:chan/{cid}:{action}` capability. A denial collapses to
//! [`ChannelError::Denied`] with no detail — a caller without access cannot tell an empty
//! channel from a forbidden one.

use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};

use super::error::ChannelError;
use super::key::cap_resource;

/// Authorize `action` (`Pub` to post, `Sub` to read/listen) on channel `cid` in workspace
/// `ws` for `principal`. `Ok(())` only if both gates pass.
pub fn authorize(
    principal: &Principal,
    ws: &str,
    cid: &str,
    action: Action,
) -> Result<(), ChannelError> {
    let req = Request::new(ws, Surface::Bus, cap_resource(cid), action);
    match check(principal, &req) {
        Decision::Allowed => Ok(()),
        Decision::Denied(_) => Err(ChannelError::Denied),
    }
}
