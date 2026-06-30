//! The [`AgentWrapper`](crate::wrapper::AgentWrapper) impls — one file per agent. [`vtcode`] is the
//! shipped, real-binary-exercised reference; [`codex`] is a **future example** (not an integration
//! yet) that keeps the seam honest about a structurally different agent. To add an agent (pi.dev,
//! dirge, claude-code, …): drop a `wrappers/<agent>.rs`, implement the trait, and re-export it here.
//! Nothing else changes — the driver is generic over the trait. This folder is the literal expression
//! of the topic's "the unit we adopt is a protocol/seam, not an agent".

pub mod codex;
pub mod vtcode;

pub use codex::CodexWrapper;
pub use vtcode::VtcodeWrapper;
