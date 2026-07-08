//! `lb-external-agent` — a **standalone, not-yet-integrated** driver for third-party coding agents,
//! proving the external-agent topic's core seam against *real* subprocesses
//! (`docs/scope/external-agent/`).
//!
//! The topic's thesis (umbrella scope): **the seam is `AgentRuntime`; the wire is ACP; the wall is
//! MCP.** **This crate is a cheaper seam-proof, NOT that ACP wire.** It drives `vtcode exec --json`
//! over NDJSON **stdout** (no ACP, no SDK — `Cargo.toml` is `lb-run-events` + serde/tokio/thiserror),
//! to validate the two halves cheapest to get wrong before paying for the ACP transport: the per-agent
//! launch/decode seam and the wire→`RunEvent` projection. The unit it adopts is the
//! **[`AgentWrapper`](wrapper::AgentWrapper) seam, not any one agent**: the [`driver`] is generic over
//! the trait, and each agent is a thin file under [`wrappers`] ([`wrappers::VtcodeWrapper`] is the
//! shipped reference, driven against a real binary; [`wrappers::CodexWrapper`] is a **future example** —
//! not an integration yet, present only to prove the seam accounts for a structurally different second
//! agent: adding one is a new file, never a driver change).
//!
//! **What carries over to the ACP driver (#2 proper), and what does not.** The `AgentProfile`-as-data
//! design and the `RunEvent` projection **target** carry over unchanged. The **transport does not**:
//! moving to ACP adds the JSON-RPC stdio transport, the `initialize` handshake + capability
//! advertisement, the `-rmcp` MCP bridge, and a `SessionNotification`-based decode that **replaces**
//! each wrapper's line-based `decode_line`. So this is the seam-proof whose transport is re-pointed onto
//! the SDK — not a drop-in `AcpRuntime` body. (See `docs/scope/external-agent/acp-driver-scope.md`
//! "Implementation status".)
//!
//! **Intentionally absent (not integrated yet, per the ask):** the host trait + feature wiring (#1),
//! the capability wall / built-ins-off sandbox (#3), gateway model routing (#4), and the durable
//! job / resume / supervision (#5). Nothing in the node depends on this crate; it is an add-on a later
//! slice plugs into the seam. Keeping it a leaf is what keeps the future OFF build clean.

pub mod driver;
pub mod profile;
pub mod wrapper;
pub mod wrappers;

pub use driver::{drive, DriveError};
pub use profile::{AgentProfile, ModelEndpoint};
pub use wrapper::{AgentWrapper, Decoded, McpConfig};
pub use wrappers::{CodexWrapper, VtcodeWrapper};
