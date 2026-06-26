//! The MCP tool layer (README §6.5, mcp scope). Exposes the tools of the extensions hosted on
//! this node under one namespace `<extension>.<tool>`, and runs the call pipeline:
//!
//! ```text
//! call ─> resolve ─> authorize (caps::check — DENY here) ─> dispatch (runtime) ─> result
//! ```
//!
//! `authorize` is the *only* gate to dispatch and calls the same `caps::check` chokepoint as
//! every other surface — MCP is not special. Workspace isolation is gate 1 there; the
//! missing-grant deny is gate 2. In S1 dispatch is local; the seam is shaped so S3 can route
//! to a remote node over a Zenoh queryable without touching callers or `authorize`.

mod call;
mod registry;
mod route;
mod serve;

pub use call::{authorize_tool, call, ToolError};
pub use registry::{Hosted, Registry, Target};
pub use route::{call_key, CallReply, CallRequest};
pub use serve::serve_call;
