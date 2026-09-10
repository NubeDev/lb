//! **The token principal** — who a contractor is while they hold a link, and how narrow that is
//! (case-plane scope §"Gateway — the token principal").
//!
//! A presented token resolves to a principal with:
//!   - `sub = "party:{id}"` — the audit trail says a *party* acted, not a login they never had;
//!   - exactly two caps: `mcp:case.request.view:call` and `mcp:case.request.reply:call`;
//!   - a **constraint naming the one request id**.
//!
//! It is a **narrowing** of the caps wall, not a bypass: workspace isolation runs first as it does
//! for everyone, the two verbs re-check their own cap, and every other verb on the node is `Denied`
//! because the principal simply does not hold its capability.
//!
//! **What `constraint` can and cannot do — read this before changing the scoping.** `constraint` is
//! a *capability set*, intersected with `caps` by `lb_caps::check`; it has no notion of a record id
//! and cannot narrow a verb to one row. So the record narrowing is enforced **inside**
//! `case.request.view` / `case.request.reply` ([`request_id_in_scope`]), and the constraint carries
//! the id as a marker string ([`REQUEST_SCOPE_PREFIX`]) that the gate ignores and the verbs read.
//! Inventing a second gate mechanism for this would put a record-level wall in the one place the
//! whole system's authorization is centralised, for one route.
//!
//! The marker is *required*, not merely checked: a caller with no request marker is refused by both
//! verbs. That is what makes them token-only by construction rather than by the accident that no
//! role bundle carries their caps.

use lb_auth::Principal;

use super::error::CaseSvcError;

/// The two capabilities a token principal holds, and the ONLY two.
pub const VIEW_CAP: &str = "mcp:case.request.view:call";
pub const REPLY_CAP: &str = "mcp:case.request.reply:call";

/// The subject prefix for a party actor. `case_event.actor` is `user: | team: | party: | system:`
/// precisely so a contractor's reply is attributed to the company, not to a login.
pub const PARTY_SUB_PREFIX: &str = "party:";

/// The constraint marker naming the one request a token principal may touch. Not a capability: it
/// matches no `Request`, so it is inert at the gate and meaningful only to the two verbs.
pub const REQUEST_SCOPE_PREFIX: &str = "case_request:";

/// Mint the token principal for `party_id` on `request_id` in `ws`.
///
/// Built through `routed` + `derive` because `derive` is the only public way to set a constraint,
/// and going through it also stamps the delegation bound — so even a future verb that widened this
/// principal's caps could not exceed the three strings below.
pub(super) fn token_principal(ws: &str, party_id: &str, request_id: &str) -> Principal {
    let sub = format!("{PARTY_SUB_PREFIX}{party_id}");
    let bound = Principal::routed(
        &sub,
        ws,
        vec![
            VIEW_CAP.to_string(),
            REPLY_CAP.to_string(),
            format!("{REQUEST_SCOPE_PREFIX}{request_id}"),
        ],
    );
    bound.derive(&sub, vec![VIEW_CAP.to_string(), REPLY_CAP.to_string()])
}

/// The request id this principal is scoped to, if any.
pub(super) fn scoped_request_of(principal: &Principal) -> Option<&str> {
    principal
        .constraint()?
        .iter()
        .find_map(|c| c.strip_prefix(REQUEST_SCOPE_PREFIX))
}

/// Refuse unless `principal` is scoped to exactly `request_id`.
///
/// Both failure modes are the same opaque [`CaseSvcError::Denied`]: "you hold no request scope" and
/// "you hold a scope for a different request" must not be distinguishable, or the reply route
/// becomes an oracle for which request ids exist.
pub(super) fn request_id_in_scope(
    principal: &Principal,
    request_id: &str,
) -> Result<(), CaseSvcError> {
    match scoped_request_of(principal) {
        Some(scoped) if scoped == request_id => Ok(()),
        _ => Err(CaseSvcError::Denied),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_caps::{check, Action, Decision, Request, Surface};

    fn allows(principal: &Principal, ws: &str, tool: &str) -> bool {
        matches!(
            check(
                principal,
                &Request::new(ws, Surface::Mcp, tool, Action::Call)
            ),
            Decision::Allowed
        )
    }

    #[test]
    fn the_token_principal_holds_two_verbs_and_nothing_else() {
        let p = token_principal("nube", "northern", "REQ1");
        assert!(allows(&p, "nube", "case.request.view"));
        assert!(allows(&p, "nube", "case.request.reply"));
        for other in [
            "case.get",
            "case.list",
            "case.request.send",
            "insight.raise",
            "federation.query",
            "store.query",
        ] {
            assert!(
                !allows(&p, "nube", other),
                "a token principal must not reach {other}"
            );
        }
    }

    #[test]
    fn the_workspace_wall_holds_for_a_token_principal_like_anyone_else() {
        let p = token_principal("nube", "northern", "REQ1");
        assert!(!allows(&p, "acme", "case.request.view"));
    }

    #[test]
    fn the_scope_names_one_request_and_refuses_every_other() {
        let p = token_principal("nube", "northern", "REQ1");
        assert!(request_id_in_scope(&p, "REQ1").is_ok());
        assert!(request_id_in_scope(&p, "REQ2").is_err());
        // A principal with no marker at all — e.g. an admin who somehow held the caps — is refused
        // by the SAME check, so the verbs are token-only by construction.
        let admin = Principal::routed("user:test", "nube", vec![VIEW_CAP.to_string()]);
        assert!(request_id_in_scope(&admin, "REQ1").is_err());
    }
}
