//! The extension **registry** service — pull · verify · cache · install · rollback (README §6.4,
//! registry scope). The S7 driver. It sits beside `agent/`, `channel/`, `assets/`, and `workflow/` as
//! a host service (not a wasm extension) because it must drive `caps::check`, the `Source` fetch seam,
//! the verify gate, the local cache, and the existing install/load flow — all host-internal seams.
//!
//! It holds **no durable state** of its own (stateless, §3.4): the cache, the catalog, and the install
//! record are all SurrealDB records in the workspace namespace. Roll back to a prior version and no
//! durable workspace state is lost — the running instance never held any (the stateless-extension
//! guarantee that makes pull-verify-cache the hot-reload path, §6.3/§6.4).
//!
//! The flow, one responsibility per file (FILE-LAYOUT §3):
//!   - `source`   — the `Source` fetch seam (the registry's `Target`/`ModelAccess` analogue).
//!   - `cache`    — `cache_artifact` (takes a `VerifiedArtifact`) + `read_cached` (the offline store).
//!   - `catalog`  — `record_catalog` / `list_catalog` / `resolve` (metadata, no bytes moved).
//!   - `pull`     — fetch · VERIFY · cache, serving cached offline (the load-bearing verb).
//!   - `install`  — `install_from_registry`: pull THEN the existing S4 install; rollback = prior ver.
//!   - `authorize`— the `mcp:registry.<verb>:call` gate (workspace-first), like `authorize_workflow`.
//!   - `tool`     — the `registry.*` MCP bridge (the store-only catalog reads; pull/install are typed).
//!
//! Two independent gates throughout: the **capability** gate (`authorize_registry`) and the
//! **signature** gate (`verify_artifact` inside `pull`). Granted ≠ trusted; trusted ≠ granted.

mod authorize;
mod cache;
mod catalog;
mod error;
mod install;
mod install_native;
mod pull;
mod source;
mod tool;

pub use authorize::authorize_registry;
pub use cache::{cache_artifact, read_cached};
pub use catalog::{list_catalog, record_catalog, resolve};
pub use error::RegistryServiceError;
pub use install::install_from_registry;
pub use install_native::install_native_from_registry;
pub use pull::pull;
pub use source::Source;
pub use tool::call_registry_tool;
