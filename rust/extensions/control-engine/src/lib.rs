//! `control-engine` as a library — the modules the binary (`main.rs`) wires into the supervisor loop
//! AND the integration tests (`tests/`) drive against a real gateway. Split lib+bin so the tests can
//! reach the registry verbs / resolve / host through the crate's public API (a `bin`-only crate exposes
//! none of its modules to an integration test). The binary stays a thin `main` over [`serve`].
//!
//! The seams a test uses: `HostCtx::with_parts` (a `SidecarClient` over a real spawned gateway + an
//! explicit grant), the `tools::appliance::*` registry verbs, and `resolve::resolve` — driven exactly
//! as `serve`'s control loop drives them.

pub mod appliance;
pub mod args;
pub mod engine;
pub mod host;
pub mod resolve;
pub mod serve;
pub mod tools;
pub mod watch;

// The ONE sanctioned CE stub — compiled in only for the crate's own tests OR under the `ce-fake` build
// feature the host integration test uses. Never in a shipped binary's real call path.
#[cfg(any(test, feature = "ce-fake"))]
pub mod ce_fake;
