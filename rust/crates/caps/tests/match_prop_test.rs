//! Property test for the risky wildcard invariant (testing §2 property/fuzz, auth-caps scope):
//! `*` matches EXACTLY one segment (never across a `/`), and `**` matches a trailing run.
//!
//! Done by deterministic enumeration (no RNG — testing §3 determinism) over generated segment
//! paths, asserting the invariant directly rather than via a fuzzer corpus (that lands when
//! `cargo fuzz` is wired — testing §6 open question).

use lb_caps::{matches, Action, Request, Surface};

fn cap(resource_pattern: &str) -> Vec<String> {
    vec![format!("store:{resource_pattern}:read")]
}

fn req(resource: &str) -> Request {
    Request::new("ws", Surface::Store, resource, Action::Read)
}

#[test]
fn star_matches_exactly_one_segment_never_across_slash() {
    // `a/*` must match `a/x` but NEVER `a/x/y` (that's two segments) and never `a` (zero).
    let c = cap("a/*");
    for depth in 0..5 {
        let segs: Vec<String> = (0..depth).map(|i| format!("s{i}")).collect();
        let resource = std::iter::once("a".to_string())
            .chain(segs)
            .collect::<Vec<_>>()
            .join("/");
        let got = matches(&c, &req(&resource));
        let expected = depth == 1; // exactly one segment after `a`
        assert_eq!(
            got, expected,
            "pattern a/* vs {resource}: got {got}, want {expected}"
        );
    }
}

#[test]
fn double_star_matches_trailing_run_including_empty() {
    // `a/**` matches `a`, `a/x`, `a/x/y/z` — the whole trailing run, including zero segments.
    let c = cap("a/**");
    for depth in 0..6 {
        let segs: Vec<String> = (0..depth).map(|i| format!("s{i}")).collect();
        let resource = std::iter::once("a".to_string())
            .chain(segs)
            .collect::<Vec<_>>()
            .join("/");
        assert!(matches(&c, &req(&resource)), "a/** should match {resource}");
    }
    // but a different head never matches.
    assert!(!matches(&c, &req("b/x/y")));
}

#[test]
fn literal_segments_must_match_in_full() {
    let c = cap("a/b/c");
    assert!(matches(&c, &req("a/b/c")));
    assert!(!matches(&c, &req("a/b"))); // too short
    assert!(!matches(&c, &req("a/b/c/d"))); // too long
    assert!(!matches(&c, &req("a/x/c"))); // middle differs
}
