//! MANDATORY capability-deny test (testing §2.1): without the grant, the call is refused.
//! Exercises the second gate of `caps::check` through a real verified principal.

use lb_auth::{mint, verify, Claims, Role, SigningKey};
use lb_caps::{check, Action, Decision, Denied, Request, Surface};

/// Factory: a verified principal in `ws` holding exactly `caps` (testing §3 fixtures).
fn principal_with(ws: &str, caps: &[&str]) -> lb_auth::Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: 100,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("freshly minted token verifies")
}

#[test]
fn denies_tool_call_without_grant() {
    let p = principal_with("acme", &[]); // holds NO capabilities
    let req = Request::new("acme", Surface::Mcp, "hello.echo", Action::Call);
    assert_eq!(check(&p, &req), Decision::Denied(Denied::Capability));
}

#[test]
fn denies_when_only_a_different_tool_is_granted() {
    let p = principal_with("acme", &["mcp:hello.other:call"]);
    let req = Request::new("acme", Surface::Mcp, "hello.echo", Action::Call);
    assert_eq!(check(&p, &req), Decision::Denied(Denied::Capability));
}

#[test]
fn allows_tool_call_with_grant() {
    let p = principal_with("acme", &["mcp:hello.echo:call"]);
    let req = Request::new("acme", Surface::Mcp, "hello.echo", Action::Call);
    assert_eq!(check(&p, &req), Decision::Allowed);
}

#[test]
fn denies_store_write_when_only_read_granted() {
    let p = principal_with("acme", &["store:note:read"]);
    let req = Request::new("acme", Surface::Store, "note", Action::Write);
    assert_eq!(check(&p, &req), Decision::Denied(Denied::Capability));
}
