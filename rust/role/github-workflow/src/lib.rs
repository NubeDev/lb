//! Role: the **github-workflow background driver** — the long-running service that turns the coding
//! workflow's durable-scan verbs into a running process. It closes the gap between "the loop is proven
//! in tests" and "a node actually runs it": on a tick it drives the host's `react_to_approvals` (auto-
//! start jobs on approval) and `relay_outbox` (deliver PR/comment effects), per configured workspace.
//!
//! The host owns the verbs (stateless functions over durable sets); this crate owns only the *loop*
//! and the per-workspace bindings — the same layering as `lb-role-gateway` (host owns the MCP
//! pipeline, the gateway owns the HTTP server) and `lb-role-github-webhook` (host owns
//! `ingest_via_bridge`, the webhook owns the HTTP edge). It has **no network deps**: the delivery
//! `Target` (the GitHub HTTP client, `lb-role-github-target`) is supplied by the caller behind the
//! host trait, so this stays a pure orchestration loop. Roles depend on host, never the reverse.
//!
//! Config, not a code branch (symmetric nodes, §3.1): which workspaces the loop services + the tick
//! interval are the binary's configuration; mounting the loop is the thin role-aware wiring §3.1
//! permits in the binary, never an `if cloud` in a core crate.

mod binding;
mod directory_drive;
mod drive;

pub use binding::WorkflowBinding;
pub use directory_drive::{drive_directory_once, run_directory_loop};
pub use drive::{drive_once, run_workflow_loop, Tick};
