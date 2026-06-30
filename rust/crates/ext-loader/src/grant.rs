//! Compute the granted capability set for an extension instance.
//!
//! `granted = requested ∩ admin_approved`. This is the enforcement point of the §6.4 trust
//! model: an extension may *request* anything, but only what the workspace admin approved at
//! install becomes live. A requested-but-unapproved capability is simply absent — there is no
//! path by which the manifest alone grants a capability (the §11.5 blast-radius rule).

use crate::manifest::Manifest;

/// Intersect the manifest's requested caps with the admin-approved set. Order follows the
/// request; an exact-string match that the admin approved is granted.
///
/// For the `net:*` surface (datasources scope) the admin approves a SPECIFIC endpoint
/// (`net:tls:tsdb.acme:5432:connect`) while the manifest can only request a STATIC pattern
/// (`net:tls:*:*:connect`). So a net approval is granted when some requested net cap *pattern-covers*
/// it (the lb-caps grammar) — the approved specific string becomes the live grant. This is the
/// per-endpoint approval the scope requires (a source whose endpoint the admin did not approve is
/// never granted, even though the manifest requested the wildcard). Non-net caps keep exact-match.
pub fn grant(manifest: &Manifest, admin_approved: &[String]) -> Vec<String> {
    let mut granted = Vec::new();

    // Exact matches (every surface): the admin approved exactly what the manifest requested.
    for c in &manifest.requested_caps {
        if admin_approved.iter().any(|a| a == c) {
            granted.push(c.clone());
        }
    }

    // Net per-endpoint: a specific approved `net:*` covered by a requested net pattern is granted.
    for approved in admin_approved {
        if !approved.starts_with("net:") || granted.iter().any(|g| g == approved) {
            continue;
        }
        if net_approval_covered(&manifest.requested_caps, approved) {
            granted.push(approved.clone());
        }
    }

    granted
}

/// Is a specific approved `net:*` capability covered by some requested net pattern? Net caps are
/// matched on their CANONICAL colon form (`net:tls:HOST:PORT:connect`) — split on `:` with a
/// per-part `*` wildcard. This is dot-safe (a hostname/IP keeps its dots), unlike the generic caps
/// grammar which also splits a resource on `.`.
fn net_approval_covered(requested: &[String], approved: &str) -> bool {
    requested
        .iter()
        .filter(|c| c.starts_with("net:"))
        .any(|pattern| net_pattern_covers(pattern, approved))
}

/// Does requested net `pattern` cover the specific `approved` net cap? Both are 5-part colon caps;
/// each pattern part is a literal or `*`.
fn net_pattern_covers(pattern: &str, approved: &str) -> bool {
    let p: Vec<&str> = pattern.split(':').collect();
    let a: Vec<&str> = approved.split(':').collect();
    if p.len() != 5 || a.len() != 5 {
        return false;
    }
    p.iter()
        .zip(a.iter())
        .all(|(pp, aa)| *pp == "*" || pp == aa)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Manifest, Visibility};

    fn m(requested: &[&str]) -> Manifest {
        Manifest {
            id: "hello".into(),
            version: "0.1.0".into(),
            tier: "wasm".into(),
            world: "lazybones:ext/extension@0.1.0".into(),
            placement: "either".into(),
            requested_caps: requested.iter().map(|s| s.to_string()).collect(),
            tools: vec![],
            visibility: Visibility::Private,
            native: None,
            ui: None,
            widgets: Vec::new(),
            nodes: Vec::new(),
        }
    }

    #[test]
    fn grants_only_the_approved_intersection() {
        let manifest = m(&["store:note:read", "secret:github/token:get"]);
        let approved = vec!["store:note:read".to_string()];
        assert_eq!(
            grant(&manifest, &approved),
            vec!["store:note:read".to_string()]
        );
    }

    #[test]
    fn requested_but_unapproved_is_absent() {
        // The mandatory deny shape at the manifest layer (extensions scope testing plan).
        let manifest = m(&["secret:github/token:get"]);
        assert!(grant(&manifest, &[]).is_empty());
    }
}
