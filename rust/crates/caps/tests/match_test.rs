//! The capability grammar matcher — the worked-examples table from auth-caps scope.
//! Unit-level: pure grammar, no IO. (testing §1 Unit)

use lb_caps::{matches, Action, Request, Surface};

fn held(caps: &[&str]) -> Vec<String> {
    caps.iter().map(|s| s.to_string()).collect()
}

#[test]
fn exact_tool_call_matches() {
    let caps = held(&["mcp:hello.echo:call"]);
    let req = Request::new("acme", Surface::Mcp, "hello.echo", Action::Call);
    assert!(matches(&caps, &req));
}

#[test]
fn single_wildcard_segment_matches() {
    let caps = held(&["mcp:hello.*:call"]);
    let req = Request::new("acme", Surface::Mcp, "hello.echo", Action::Call);
    assert!(matches(&caps, &req));
}

#[test]
fn different_tool_does_not_match() {
    let caps = held(&["mcp:hello.echo:call"]);
    let req = Request::new("acme", Surface::Mcp, "hello.secret", Action::Call);
    assert!(!matches(&caps, &req));
}

#[test]
fn wrong_action_does_not_match() {
    let caps = held(&["store:note:read"]);
    let req = Request::new("acme", Surface::Store, "note", Action::Write);
    assert!(!matches(&caps, &req));
}

#[test]
fn recursive_wildcard_matches_tail() {
    let caps = held(&["bus:chan/**:sub"]);
    let req = Request::new("acme", Surface::Bus, "chan/eng/general", Action::Sub);
    assert!(matches(&caps, &req));
}

#[test]
fn any_action_wildcard_matches() {
    let caps = held(&["store:note:*"]);
    assert!(matches(
        &caps,
        &Request::new("acme", Surface::Store, "note", Action::Read)
    ));
    assert!(matches(
        &caps,
        &Request::new("acme", Surface::Store, "note", Action::Write)
    ));
}

#[test]
fn reach_surface_gates_a_named_page() {
    // nav-reach scope: a curated-nav subject holds `reach:<surface>:view` for exactly their menu's
    // surfaces. `reach:dashboards:view` reaches the Dashboards page but NOT the Rules page.
    let caps = held(&["reach:dashboards:view"]);
    assert!(matches(
        &caps,
        &Request::new("acme", Surface::Reach, "dashboards", Action::View)
    ));
    assert!(!matches(
        &caps,
        &Request::new("acme", Surface::Reach, "rules", Action::View)
    ));
}

#[test]
fn reach_wildcard_reaches_every_surface() {
    // nav-reach scope: a FALLBACK subject (no curated nav) holds the sentinel `reach:*:view`, which
    // the single-segment `*` grants for any surface — so a default member/admin is never locked out.
    let caps = held(&["reach:*:view"]);
    for surface in [
        "dashboards",
        "rules",
        "flows",
        "ingest",
        "datasources",
        "system",
    ] {
        assert!(
            matches(
                &caps,
                &Request::new("acme", Surface::Reach, surface, Action::View)
            ),
            "reach:*:view must reach surface {surface}"
        );
    }
}

#[test]
fn malformed_capability_grants_nothing() {
    // deny-by-default: an unparseable grant grants nothing (grammar.rs).
    let caps = held(&["this is not a cap", "mcp::call", ":resource:"]);
    let req = Request::new("acme", Surface::Mcp, "hello.echo", Action::Call);
    assert!(!matches(&caps, &req));
}
