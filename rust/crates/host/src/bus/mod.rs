//! The bus pub/sub service — generic, workspace-walled, capability-gated subject pub/sub (widget-config-
//! vars scope, "Platform fix"). The one missing API: today the bus is reachable only through
//! **series**-scoped verbs; this adds a generic subject surface the JSON-over-SSE sink, Zenoh-sourced
//! variables, and live events on non-series subjects need. A shared surface (not dashboard-private) — any
//! extension wanting workspace pub/sub uses the same verbs.
//!
//! Verbs (one per file, FILE-LAYOUT):
//!   - `bus.publish` ([`bus_publish`]) — fire-and-forget motion (NOT durable; must-deliver → outbox).
//!   - `bus.watch` ([`bus_watch`]) — subscribe a walled subject (stream; backs the gateway SSE route).
//!   - the wall guard ([`wall_subject`]) — namespaces `subject` to `ws/{id}/ext/{subject}` + rejects
//!     reserved prefixes (`series/`, `channels/`, internal) so a caller cannot impersonate platform
//!     motion nor escape the workspace wall (rule 6).
//!   - the MCP bridge ([`call_bus_tool`]) — `bus.publish` over `POST /mcp/call`.

mod authorize;
mod error;
mod publish;
mod subscribe;
mod tool;
mod watch;

pub use authorize::{authorize_bus, wall_subject};
pub use error::BusError;
pub use publish::bus_publish;
pub use subscribe::BusSub;
pub use tool::call_bus_tool;
pub use watch::bus_watch;
