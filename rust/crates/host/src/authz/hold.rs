//! [`holds_cap`] — does this principal hold the capability named by `cap` in `ws`? The no-widening
//! predicate: `grants.assign` / `roles.define` use it so an admin can only hand out (or bundle) caps
//! they themselves possess (authz-grants: "a custom role can only grant caps the definer holds").
//!
//! It parses the `surface:resource:action` grant string into a [`Request`] for the workspace and
//! asks `lb_caps::matches` against the principal's own caps — the same pattern-match the enforcement
//! gate uses, so "holds" here means exactly "would pass Gate 2". An unparseable cap string is held
//! by no one (deny by default).

use lb_auth::Principal;
use lb_caps::{matches, Action, Request, Surface};

/// True iff `principal` holds a capability that grants `cap` in `ws`. Unparseable `cap` → false.
pub fn holds_cap(principal: &Principal, ws: &str, cap: &str) -> bool {
    match request_for(ws, cap) {
        Some(req) => matches(principal.caps(), &req),
        None => false,
    }
}

/// Parse `surface:resource:action` into the [`Request`] it would authorize. `None` if the shape or
/// the surface/action is unknown (mirrors `Capability::parse`'s validation, deny by default).
fn request_for(ws: &str, cap: &str) -> Option<Request> {
    let mut parts = cap.splitn(3, ':');
    let surface = Surface::parse(parts.next()?)?;
    let resource = parts.next()?;
    let action_str = parts.next()?;
    if resource.is_empty() || action_str.contains(':') {
        return None;
    }
    let action = Action::parse(action_str)?;
    Some(Request::new(ws, surface, resource, action))
}
