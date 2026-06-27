//! Compute the granted capability set for an extension instance.
//!
//! `granted = requested ∩ admin_approved`. This is the enforcement point of the §6.4 trust
//! model: an extension may *request* anything, but only what the workspace admin approved at
//! install becomes live. A requested-but-unapproved capability is simply absent — there is no
//! path by which the manifest alone grants a capability (the §11.5 blast-radius rule).

use crate::manifest::Manifest;

/// Intersect the manifest's requested caps with the admin-approved set. Order follows the
/// request; only exact-string matches that the admin approved are granted.
pub fn grant(manifest: &Manifest, admin_approved: &[String]) -> Vec<String> {
    manifest
        .requested_caps
        .iter()
        .filter(|c| admin_approved.iter().any(|a| a == *c))
        .cloned()
        .collect()
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
            widget: None,
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
