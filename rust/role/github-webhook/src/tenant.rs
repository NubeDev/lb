//! The **multi-tenant front door**'s routing table: a tenant slug → its `(workspace, principal,
//! secret)`. One receiver process fronts many workspaces, each with its **own** webhook secret, so a
//! single deployed node serves every tenant without a code branch (symmetric nodes, §3.1).
//!
//! Routing is **by a URL path segment** (`POST /webhook/{tenant}`), deliberately NOT by reading the
//! repo out of the body. The per-tenant secret must be chosen *before* the HMAC check, but the repo
//! lives *inside* the signed body — a chicken-and-egg the path param sidesteps: GitHub lets each repo
//! point its webhook at a distinct Payload URL, so the tenant is known from the URL with zero trust
//! in the unverified body. Authenticity-before-parse (the §3.5 invariant) is preserved.
//!
//! The hard wall holds at the front door: the secret is looked up *per tenant*, so a delivery signed
//! with tenant A's secret but POSTed to tenant B's slug fails B's HMAC (different key) → `401`, and
//! never reaches B's workspace. An **unknown** tenant is also `401` (not `404`) on purpose — a `404`
//! would be an enumeration oracle telling a prober which tenants exist. Each tenant's `Principal` is
//! workspace-scoped, so even past the secret the `ingest_via_bridge` cap+ws gates re-check authority.
//!
//! Single-tenant [`WebhookState`](crate::WebhookState) (the `/webhook` route) stays for the one-repo
//! deployment; this is the many-repo front door layered beside it. The secret is mediated here (read
//! only by the verifier) exactly as in `WebhookState`; `lb-secrets` backing lands later.

use std::collections::HashMap;
use std::sync::Arc;

use lb_auth::Principal;
use lb_host::Node;

/// One tenant's binding: the workspace a verified delivery writes into, the principal it ingests as,
/// and the HMAC secret that authenticates *this tenant's* deliveries (the bytes from that repo's
/// webhook config). The `github-bridge` extension must be installed in `ws`.
#[derive(Clone)]
pub struct WebhookTenant {
    pub principal: Arc<Principal>,
    pub ws: String,
    secret: Arc<Vec<u8>>,
}

impl WebhookTenant {
    pub fn new(principal: Principal, ws: impl Into<String>, secret: impl Into<Vec<u8>>) -> Self {
        Self {
            principal: Arc::new(principal),
            ws: ws.into(),
            secret: Arc::new(secret.into()),
        }
    }

    /// This tenant's HMAC secret — read only by the signature check ([`crate::verify`]). Crate-private
    /// on purpose: nothing outside the verifier touches it, and it must never reach a log line.
    pub(crate) fn secret(&self) -> &[u8] {
        &self.secret
    }
}

/// The front door's routing table + the node every tenant shares. Built once at boot; cloned into
/// each request by axum (the `Arc`s keep it cheap). A tenant is addressed by an opaque slug — the
/// caller chooses the mapping (e.g. `acme-api` → the `acme` workspace), never the raw repo name, so
/// the slug can hide the tenant↔workspace relationship.
#[derive(Clone)]
pub struct TenantRegistry {
    pub node: Arc<Node>,
    tenants: Arc<HashMap<String, WebhookTenant>>,
}

impl TenantRegistry {
    /// Build a registry around a shared node and an iterator of `(slug, tenant)` bindings.
    pub fn new(
        node: Arc<Node>,
        tenants: impl IntoIterator<Item = (String, WebhookTenant)>,
    ) -> Self {
        Self {
            node,
            tenants: Arc::new(tenants.into_iter().collect()),
        }
    }

    /// Resolve a tenant slug to its binding, or `None` if no such tenant — the handler maps `None` to
    /// an opaque `401` (no enumeration oracle), the same status a bad signature returns.
    pub(crate) fn resolve(&self, slug: &str) -> Option<&WebhookTenant> {
        self.tenants.get(slug)
    }
}
