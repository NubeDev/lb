//! The POSITIVE gate tests — the caps exist in the admin bundle, in neither lesser one, and both verbs are advertised.
//!
//! Part of the `sla_policy` suite (see `sla_policy_support.rs` for the fixtures and the
//! full preamble). One binary: `case_suite.rs`.

use super::sla_policy_support::*;

// --- MANDATORY: the POSITIVE gate test ------------------------------------------------------------

/// **A shipped-but-unusable verb is the failure mode this test exists for.** A capability that
/// exists in NO role bundle makes its verb `Denied` for every caller including admins, and every
/// negative test above still passes — they assert a deny, and a deny is exactly what a missing cap
/// produces. `media.upload_*` and `series.retention.delete` both shipped in that state and were
/// found on a live node.
///
/// So: mint a token carrying the REAL `workspace-admin` bundle (not a hand-written cap list) and
/// prove both verbs actually run. If either cap ever falls out of `ADMIN_ONLY_CAPS`, this goes red.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_real_workspace_admin_bundle_can_reach_both_verbs() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let bundle = workspace_admin_role_caps();
    assert!(
        bundle.iter().any(|c| c == SET) && bundle.iter().any(|c| c == LIST),
        "both caps must exist in the shipped workspace-admin bundle, or the verbs are \
         unreachable for EVERY caller: {bundle:?}"
    );

    let caps: Vec<&str> = bundle.iter().map(String::as_str).collect();
    let admin = principal("user:admin", "nube", &caps);
    call(
        &node,
        &admin,
        "nube",
        "policy.sla.set",
        policy("default", json!({})),
    )
    .await
    .expect("a real admin bundle can SET");
    let out = call(&node, &admin, "nube", "policy.sla.list", json!({}))
        .await
        .expect("a real admin bundle can LIST");
    assert_eq!(ids(&out), ["default"]);
}

/// The other half of the tiering: neither cap leaks into the member or viewer bundle, and no
/// wildcard in those bundles spans them. Asserted against the REAL bundles, so a future edit that
/// widens `member` cannot quietly hand the commercial terms to everyone.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn neither_cap_is_in_the_member_or_viewer_bundle() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    for (tier, bundle) in [
        ("member", member_role_caps()),
        ("viewer", viewer_role_caps()),
    ] {
        for cap in [SET, LIST] {
            assert!(
                !bundle.iter().any(|c| c == cap),
                "{cap} must not be in the {tier} bundle"
            );
        }
        // And the bundle as a WHOLE — wildcards included — cannot reach either verb.
        let caps: Vec<&str> = bundle.iter().map(String::as_str).collect();
        let p = principal("user:tiered", "nube", &caps);
        for tool in ["policy.sla.set", "policy.sla.list"] {
            assert!(
                matches!(
                    call(&node, &p, "nube", tool, policy("x", json!({}))).await,
                    Err(ToolError::Denied)
                ),
                "the full {tier} bundle must not reach {tool} (check for a spanning wildcard)"
            );
        }
    }
}

/// **Dispatchable but invisible is its own kind of broken.** The console and the agent's menu are
/// both built from `tools.catalog`, so a verb missing from the host catalog can be called only by
/// someone who already knows its name — which is how whole families (`datasource.`, `viz.`,
/// `flows.`) once went missing. An admin must SEE both verbs.
///
/// The catalog is gated by the same `gate_tool_for` the dispatcher uses, so this also re-proves the
/// cardinal rule from the other side: advertise a tool only if the call would allow it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_admin_sees_both_verbs_in_the_tools_catalog() {
    let node = Node::boot().await.expect("node boots");
    let mut caps: Vec<String> = workspace_admin_role_caps();
    caps.push("mcp:tools.catalog:call".into());
    let refs: Vec<&str> = caps.iter().map(String::as_str).collect();
    let admin = principal("user:admin", "nube", &refs);

    let catalog = lb_host::tools_catalog(&node, &admin, "nube")
        .await
        .expect("catalog");
    for verb in ["policy.sla.set", "policy.sla.list"] {
        assert!(
            catalog.tools.iter().any(|d| d.name == verb),
            "{verb} is dispatchable but absent from tools.catalog — invisible to the console \
             and the agent menu"
        );
    }
}
