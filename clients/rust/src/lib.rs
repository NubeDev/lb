//! `lb-client` — a thin external client for a Lazybones gateway node.
//!
//! The five-method shape (mirrored across the four language clients under
//! `clients/`): construct a `Client` with a base URL + a bearer, then call
//! `write_samples` / `latest_sample` / `call_mcp` / `sign_webhook` /
//! `post_webhook`. The bearer is EITHER an API key (`lbk_{ws}.{id}.{secret}`)
//! OR a JWT from `/login`; this library does not branch on which — the gateway
//! already splits on the `lbk_` prefix in one place
//! (`rust/role/gateway/src/session/authenticate.rs`).
//!
//! See `README.md` for the auth + round-trip walkthrough, and
//! `docs/scope/clients/client-libraries-scope.md` for the design.

mod client;
mod error;
mod ingest;
mod mcp;
mod webhook;

pub use client::{Client, LoginReply};
pub use error::{ApiError, LbError};
pub use ingest::{latest_sample, write_samples, LatestSampleReply, WriteSamplesReply};
pub use mcp::call_mcp;
pub use webhook::{post_webhook, sign_webhook};

pub use serde_json::Value as Json;
