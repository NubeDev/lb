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
//! Why a role crate (not core `lb-host`): a webhook receiver pulls in `axum` (an HTTP server) and
//! `hmac` — neither belongs compiled into every node. It lives beside `lb-role-registry-host`, the
//! same way the registry's HTTP server does. Roles depend on host, never the reverse.

mod routes;
mod server;
mod state;
mod verify;

pub use server::{router, serve};
pub use state::WebhookState;
pub use verify::{verify_signature, SignatureError};
