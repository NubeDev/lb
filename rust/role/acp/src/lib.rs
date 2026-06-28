//! Role-only: the **ACP stdio adapter** (agent-run scope Part 4) — lets Zed / Cursor / any
//! Agent-Client-Protocol host drive the central agent. It is a binary like `role/gateway`: it
//! authenticates a trusted local session (a real `lb_auth` token bound to one workspace — never a
//! bypass) and translates the ACP v1 turn lifecycle onto the host's run primitives (Part 0–3).
//!
//! It is a **thin encoder**, not a kernel change: the stable internal contract is the durable
//! transcript (Part 0) + the `RunEvent` vocabulary (Part 1); this crate is a pure
//! `RunEvent <-> ACP` mapping (`encode.rs`) plus the lifecycle driver (`session.rs`) and a stdio
//! transport (`stdio.rs`). Nothing in the loop knows the word "ACP" — symmetric with how the gateway
//! SSE route is *another* encoder over the same vocabulary (rule 1: one model, many projections).
//!
//! Split into a lib + a `lb-acp` bin so a test can drive the driver in-process against a real Node
//! AND spawn the real binary over a real stdio pipe (no fakes — rule 9).

mod encode;
mod rpc;
mod session;
mod stdio;

pub use encode::{encode_update, stop_reason};
pub use rpc::{codes, ErrorResponse, Notification, Request, Response};
pub use session::{AcpSession, DriverError, Handled};
pub use stdio::serve_stdio;
