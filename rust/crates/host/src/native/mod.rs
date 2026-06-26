//! The native **Tier-2** service — supervise an OS child process beside the wasm tier (README §6.3,
//! native-tier scope). The S7 exit gate's second half: a native sidecar is supervised and restarts
//! cleanly. It sits beside `agent/`, `channel/`, `assets/`, `workflow/`, `registry/` as a host
//! service (not an extension) because it must drive `caps::check`, the supervisor `Launcher` seam,
//! the durable records, and identity minting — all host-internal seams.
//!
//! It holds **no durable state** of its own (§3.4): the install record + the `native_status`
//! projection are SurrealDB records in the workspace namespace; the live `Sidecar` (PID, stdio) is a
//! runtime-only cache the records can rebuild. Kill a child and respawn it and no durable workspace
//! state is lost — the running child never held any (the stateless-extension guarantee carried into
//! Tier 2).
//!
//! The flow, one responsibility per file (FILE-LAYOUT §3):
//!   - `registry`  — the runtime `SidecarMap` (live children, keyed `(ws, ext_id)`; never the store).
//!   - `status`    — the durable `native_status` projection (lifecycle intent + restart count).
//!   - `spec`      — build a `lb_supervisor::Spec` from a manifest + inject the scoped identity token.
//!   - `authorize` — the `mcp:native.<verb>:call` gate (workspace-first), like `authorize_registry`.
//!   - `install`   — `install_native`: persist records → spawn → supervise (the start verb).
//!   - `lifecycle` — `stop` / `restart` / `status` (the operator controls).
//!   - `tool`      — the `native.*` MCP bridge (store-only `status`) + `call_sidecar` (child dispatch
//!                   with crash-restart-on-fault — the supervision proof).
//!
//! Two independent gates throughout: the **capability** gate (`authorize_native`) and, when the
//! binary came from the signed registry, the **signature** gate (`verify_artifact` in `pull`).
//! Granted ≠ trusted; trusted ≠ granted — carried verbatim from the wasm/registry tiers.

mod authorize;
mod error;
mod install;
mod lifecycle;
mod registry;
mod spec;
mod status;
mod tool;

pub use authorize::authorize_native;
pub use error::NativeServiceError;
pub use install::{install_native, Supervised};
pub use lifecycle::{restart_native, status_native, stop_native};
pub use registry::SidecarMap;
pub use spec::build_spec;
pub use status::{read_status, Lifecycle, NativeStatus};
pub use tool::{call_native_tool, call_sidecar};
