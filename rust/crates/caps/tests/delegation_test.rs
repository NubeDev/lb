//! Delegation = the agent's derived principal acts under `agent ∩ caller` (agent + auth-caps
//! scopes). `Principal::derive` can only NARROW: the derived actor is bounded by BOTH its own caps
//! and the caller's. These prove the intersection holds in both directions — the security-critical
//! property that an agent can never widen its access (agent scope "the intersection is the gate").

use lb_auth::{mint, verify, Claims, Role, SigningKey};
use lb_caps::{check, Action, Decision, Denied, Request, Surface};

fn caller(ws: &str, caps: &[&str]) -> lb_auth::Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:ada".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: 100,
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

#[test]
fn delegated_actor_can_do_what_both_sides_grant() {
    let caller = caller("acme", &["mcp:hello.echo:call", "store:doc/*:read"]);
    // The agent itself holds the echo cap; derive intersects with the caller's caps.
    let agent = caller.derive("agent:summarize", vec!["mcp:hello.echo:call".into()]);
    let req = Request::new("acme", Surface::Mcp, "hello.echo", Action::Call);
    assert_eq!(check(&agent, &req), Decision::Allowed);
}

#[test]
fn delegated_actor_cannot_use_a_cap_the_caller_lacks_even_if_the_agent_holds_it() {
    // The AGENT lists a broad cap, but the CALLER does not hold it → the intersection denies it.
    // This is the no-widening guarantee: invoking the agent never escalates the caller's access.
    let caller = caller("acme", &["mcp:hello.echo:call"]); // caller can NOT write docs
    let agent = caller.derive("agent:x", vec!["store:doc/*:write".into()]); // agent claims it
    let req = Request::new("acme", Surface::Store, "doc/secret", Action::Write);
    assert_eq!(
        check(&agent, &req),
        Decision::Denied(Denied::Capability),
        "agent must NOT widen beyond the caller"
    );
}

#[test]
fn delegated_actor_cannot_use_a_cap_the_agent_lacks_even_if_the_caller_holds_it() {
    // The CALLER holds a broad cap, but the AGENT was not delegated it → still denied. The agent's
    // own grant is the other half of the intersection (least privilege from both directions).
    let caller = caller("acme", &["store:doc/*:read", "store:doc/*:write"]);
    let agent = caller.derive("agent:x", vec!["store:doc/*:read".into()]); // read only
    let req = Request::new("acme", Surface::Store, "doc/x", Action::Write);
    assert_eq!(check(&agent, &req), Decision::Denied(Denied::Capability));
}

#[test]
fn delegation_cannot_cross_the_workspace_wall() {
    // `derive` inherits the caller's ws and cannot change it; a request targeting another ws is
    // refused at gate 1 (the hard wall holds for delegated actors too, §3.6).
    let caller = caller("acme", &["mcp:hello.echo:call"]);
    let agent = caller.derive("agent:x", vec!["mcp:hello.echo:call".into()]);
    let req = Request::new("other-ws", Surface::Mcp, "hello.echo", Action::Call);
    assert_eq!(check(&agent, &req), Decision::Denied(Denied::Workspace));
}
