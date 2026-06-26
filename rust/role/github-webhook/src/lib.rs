//! Role: the **GitHub webhook receiver** — the live HTTP ingress for the coding workflow's inbound
//! edge. It completes the path the `github-bridge` slice deliberately left out: that slice shipped
//! `lb_host::ingest_via_bridge` (a typed host helper a test/UI drove), and this crate is the real
//! HTTP POST that drives it from an actual GitHub webhook delivery.
//!
//! The receiver is a node (symmetric nodes, §3.1) that also exposes one inbound route. It adds no
//! authority of its own; the security boundary is two layers:
//!   1. **Transport authenticity** ([`verify`]) — `HMAC-SHA256(secret, raw-body)` against
//!      `X-Hub-Signature-256`, constant-time, over the exact bytes GitHub signed. This proves the
//!      delivery came from the secret-holder; the secret is mediated ([`WebhookState`]) and never
//!      logged. It is layered BEFORE, never instead of, the capability gates.
//!   2. **Capability + workspace** — a verified delivery calls `ingest_via_bridge` under a fixed
//!      principal/workspace, so the SAME two host gates (`mcp:github-bridge.normalize:call`, then
//!      `mcp:workflow.ingest_issue:call`) and the workspace wall guard a webhook as guard every
//!      other caller. Idempotency on the issue id (the inbox upsert) makes re-delivery one item.
//!
//! Two front-door shapes share that boundary: the single-tenant [`router`] (`POST /webhook`, one
//! fixed `(ws, principal, secret)`) for a one-repo deployment, and the **multi-tenant**
//! [`tenant_router`] (`POST /webhook/{tenant}`, a [`TenantRegistry`] of slug → `(ws, principal,
//! secret)`) so one process fronts many workspaces, each with its own secret. Routing is by URL slug,
//! not by the unverified body, so the per-tenant secret is chosen *before* the HMAC check — and a
//! delivery signed with one tenant's secret can never cross into another's workspace (an unknown
//! tenant is an opaque `401`, no enumeration oracle). See [`tenant`].
//!
//! Why a role crate (not core `lb-host`): a webhook receiver pulls in `axum` (an HTTP server) and
//! `hmac` — neither belongs compiled into every node. It lives beside `lb-role-registry-host`, the
//! same way the registry's HTTP server does. Roles depend on host, never the reverse.

mod route_tenant;
mod routes;
mod server;
mod state;
mod tenant;
mod verify;

pub use server::{router, serve, serve_tenants, tenant_router};
pub use state::WebhookState;
pub use tenant::{TenantRegistry, WebhookTenant};
pub use verify::{verify_signature, SignatureError};
