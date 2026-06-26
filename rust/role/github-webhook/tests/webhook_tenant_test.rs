//! The **multi-tenant front door** — `POST /webhook/{tenant}` routes one process's deliveries to many
//! workspaces, each authenticated by its OWN secret. The mandatory workspace-isolation category lives
//! here at its sharpest: a delivery signed with tenant A's secret but POSTed to tenant B's slug is
//! rejected at B's HMAC check, never crossing into B's workspace.
//!
//! One node fronts two tenants (`acme` and `globex`), the real `github-bridge` installed in each.
//! Drives `tenant_router` via `tower::oneshot` (no socket) — the inbox in each workspace is the
//! side-effect under test.

mod common;

use std::sync::Arc;

use common::*;
use lb_host::Node;
use lb_role_github_webhook::TenantRegistry;

/// Secrets are per-tenant — that is the whole point of the front door.
const ACME_SECRET: &[u8] = b"acme-webhook-secret";
const GLOBEX_SECRET: &[u8] = b"globex-webhook-secret";

/// Boot one node, install the bridge in both workspaces, and build a two-tenant registry. The slugs
/// (`acme-api`, `globex-app`) are opaque — they need not equal the workspace id.
async fn two_tenant_front_door() -> (Arc<Node>, TenantRegistry) {
    let node = Arc::new(Node::boot().await.unwrap());
    install_bridge(&node, "acme").await.unwrap();
    install_bridge(&node, "globex").await.unwrap();
    let registry = TenantRegistry::new(
        node.clone(),
        [
            (
                "acme-api".to_string(),
                tenant("acme", &ingest_caps(), ACME_SECRET),
            ),
            (
                "globex-app".to_string(),
                tenant("globex", &ingest_caps(), GLOBEX_SECRET),
            ),
        ],
    );
    (node, registry)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_tenant_routes_its_delivery_to_its_own_workspace() {
    // HAPPY PATH, two tenants: each repo points at its own /webhook/{slug} with its own secret; each
    // delivery lands an item in ITS workspace's triage inbox — and only there.
    let (node, registry) = two_tenant_front_door().await;

    let acme_body = issue_opened_webhook(1);
    let st = tenant_status(
        registry.clone(),
        signed_tenant_req("acme-api", &acme_body, ACME_SECRET),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "acme delivery ingested");

    let globex_body = issue_opened_webhook(2);
    let st = tenant_status(
        registry,
        signed_tenant_req("globex-app", &globex_body, GLOBEX_SECRET),
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK, "globex delivery ingested");

    // Each workspace's triage inbox has exactly its own item — never the other's.
    let acme_items = lb_inbox::list(&node.store, "acme", "triage").await.unwrap();
    let globex_items = lb_inbox::list(&node.store, "globex", "triage")
        .await
        .unwrap();
    assert_eq!(acme_items.len(), 1, "one item in acme");
    assert_eq!(globex_items.len(), 1, "one item in globex");
    // The bridge keys the item by issue id; both used #2451, but they live in separate namespaces.
    assert_ne!(
        acme_items[0].id, "",
        "acme has its own item id in its own namespace"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_delivery_signed_with_another_tenants_secret_is_rejected() {
    // THE ISOLATION HEADLINE: an authentic-looking delivery for the `globex-app` slug, but signed
    // with ACME's secret, fails globex's HMAC → 401. It never reaches globex's workspace. The
    // per-tenant secret is the wall at the front door.
    let (node, registry) = two_tenant_front_door().await;

    let body = issue_opened_webhook(1);
    let st = tenant_status(
        registry,
        // Correct signature, WRONG secret for this tenant slug.
        signed_tenant_req("globex-app", &body, ACME_SECRET),
    )
    .await;
    assert_eq!(
        st,
        axum::http::StatusCode::UNAUTHORIZED,
        "cross-tenant secret is rejected, not crossed"
    );
    assert!(
        lb_inbox::list(&node.store, "globex", "triage")
            .await
            .unwrap()
            .is_empty(),
        "nothing landed in globex — the wall held at the secret"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_unknown_tenant_is_an_opaque_401_not_a_404() {
    // No enumeration oracle: a slug that isn't a tenant returns the SAME 401 a forgery does — a
    // prober cannot tell "wrong signature" from "no such tenant".
    let (_node, registry) = two_tenant_front_door().await;

    let body = issue_opened_webhook(1);
    // Even a body signed under a real secret can't help — the slug resolves to no tenant.
    let st = tenant_status(
        registry,
        signed_tenant_req("does-not-exist", &body, ACME_SECRET),
    )
    .await;
    assert_eq!(
        st,
        axum::http::StatusCode::UNAUTHORIZED,
        "unknown tenant is 401, not 404"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_authentic_delivery_to_an_ungranted_tenant_is_denied() {
    // MANDATORY capability-deny (§2.1) at the front door: a tenant whose principal lacks the ingest
    // caps is authentic (correct secret) but `403` — the cap gate inside `ingest_via_bridge` refuses.
    // Distinct from the 401: the secret is right, the authority is not.
    let node = Arc::new(Node::boot().await.unwrap());
    install_bridge(&node, "nocaps").await.unwrap();
    let registry = TenantRegistry::new(
        node.clone(),
        // A tenant with NO caps at all.
        [(
            "nocaps-repo".to_string(),
            tenant("nocaps", &[], ACME_SECRET),
        )],
    );

    let body = issue_opened_webhook(1);
    let st = tenant_status(
        registry,
        signed_tenant_req("nocaps-repo", &body, ACME_SECRET),
    )
    .await;
    assert_eq!(
        st,
        axum::http::StatusCode::FORBIDDEN,
        "authentic but ungranted → 403, not 401"
    );
    assert!(lb_inbox::list(&node.store, "nocaps", "triage")
        .await
        .unwrap()
        .is_empty());
}
