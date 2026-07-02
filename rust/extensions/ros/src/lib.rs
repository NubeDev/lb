//! `ros-sidecar` as a library — the modules the binary (`main.rs`) wires into the supervisor loop AND
//! the integration tests (`tests/`) drive against a real gateway. Split lib+bin so `tests/crud_test.rs`
//! can reach the handlers/host/fake through the crate's public API (a `bin`-only crate exposes none of
//! its modules to an integration test). The binary stays a thin `main` over `serve`.
//!
//! The seams a test uses: `HostCtx::with_parts` (a `SidecarClient` over a real spawned gateway + an
//! explicit grant), `RosApiFactory` (inject `RosFake` for a connection with no live box), and
//! `handlers::dispatch` (drive a verb exactly as `call.rs` does).

pub mod call;
pub mod handlers;
pub mod host;
pub mod paging;
pub mod poller;
pub mod resolve;
pub mod ros_api;
pub mod ros_client;
pub mod ros_fake;
pub mod shadow;
