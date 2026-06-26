//! MANDATORY workspace-isolation test (testing §2.2): the hard wall (gate 1) fires BEFORE any
//! capability is consulted. A principal in workspace B cannot act on workspace A — even
//! holding a matching capability, because the workspace gate is checked first (auth-caps
//! scope: enforcement order).

use lb_auth::{mint, verify, Claims, Role, SigningKey};
use lb_caps::{check, Action, Decision, Denied, Request, Surface};

fn principal_in(ws: &str, caps: &[&str]) -> lb_auth::Principal {
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
    verify(&key, &token, 1).expect("token verifies")
}

#[test]
fn workspace_b_cannot_act_on_workspace_a_even_with_capability() {
    // The principal is in workspace "b" and even holds the matching capability...
    let p = principal_in("b", &["store:note:read"]);
    // ...but the request targets workspace "a". Gate 1 (isolation) denies first.
    let req = Request::new("a", Surface::Store, "note", Action::Read);
    assert_eq!(check(&p, &req), Decision::Denied(Denied::Workspace));
}

#[test]
fn isolation_gate_precedes_capability_gate() {
    // No capability AND wrong workspace → the workspace denial wins (it is checked first),
    // proving the order, not just the outcome.
    let p = principal_in("b", &[]);
    let req = Request::new("a", Surface::Mcp, "hello.echo", Action::Call);
    assert_eq!(check(&p, &req), Decision::Denied(Denied::Workspace));
}

#[test]
fn same_workspace_with_capability_is_allowed() {
    let p = principal_in("a", &["store:note:read"]);
    let req = Request::new("a", Surface::Store, "note", Action::Read);
    assert_eq!(check(&p, &req), Decision::Allowed);
}

#[test]
fn isolation_holds_across_all_surfaces() {
    let p = principal_in(
        "b",
        &["mcp:hello.echo:call", "bus:chan/**:sub", "secret:x/y:get"],
    );
    for req in [
        Request::new("a", Surface::Mcp, "hello.echo", Action::Call),
        Request::new("a", Surface::Bus, "chan/general", Action::Sub),
        Request::new("a", Surface::Secret, "x/y", Action::Get),
    ] {
        assert_eq!(check(&p, &req), Decision::Denied(Denied::Workspace));
    }
}
