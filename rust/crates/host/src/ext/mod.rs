//! The **ext** lifecycle surface — one host service that makes the extension lifecycle a complete,
//! uniform, gateway-reachable verb set across both tiers (lifecycle-management scope). The runtime +
//! two tiers + signed registry built the *mechanisms*; this closes the matrix (`list` · `enable` ·
//! `disable` · `uninstall` + the boot `reconcile`) and dispatches by the `Install.tier` — so a caller
//! sees one surface, no `if tier` smell (the dispatch lives here, once).
//!
//! It holds **no new persistence**: the durable truth stays in the `Install` record (now carrying
//! `tier` + the `enabled` intent) + the native `status`; the live truth stays in the runtime
//! `SidecarMap`. `enable`/`disable` is **durable intent distinct from start/stop** — `disable` flips
//! the flag AND stops a running native child, and the boot `reconcile` honors `enabled` so a disabled
//! extension does not silently return after a restart.
//!
//! Verbs (one per file, FILE-LAYOUT §3): `ext.list` (`mcp:ext.list:call`), `ext.enable`/`ext.disable`
//! (`mcp:ext.disable:call`), `ext.start` (`mcp:ext.start:call`), `ext.uninstall`
//! (`mcp:ext.uninstall:call`), `ext.publish` (`mcp:ext.publish:call` — upload a signed artifact,
//! verify-before-store), plus the un-gated boot `reconcile` the node calls on start. The MCP bridge
//! ([`call_ext_tool`]) exposes the gated verbs.
//!
//! **`enable` is intent; `start` is the act** — the same split `disable`/`stop` already has.
//! `enable` marks an install runnable (and auto-startable at boot) without running it; `start` runs
//! it now, and refuses a disabled one rather than override the intent. Before `ext.start` existed
//! there was no way to start a stopped extension at all: `enable` spawned nothing, `native.restart`
//! and `native.reset` both need an existing handle, and republishing the artifact was the only way
//! back — see [`start`] for why that made a boot gap so expensive.
//!
//! **Boot bring-up is two verbs, one per tier**, both driven by the one `reconcile` plan and both
//! called by the node on start: [`load_enabled`] loads enabled **wasm** components back into the
//! runtime, and [`spawn_enabled`] respawns enabled **native** children through the `Launcher` the node
//! owns. Neither is capability-gated (a node-boot operation, not a caller verb). A node that calls
//! only one of them silently strands the other tier's extensions — the shape of issue #64.

mod boot_load;
mod boot_spawn;
mod boot_workspaces;
mod enable;
mod error;
mod install_dir;
mod list;
mod publish;
mod reconcile;
mod row;
mod start;
mod tool;
mod uninstall;

pub use boot_load::{load_enabled, LoadedExt};
pub use boot_spawn::{spawn_enabled, SpawnedExt};
pub use boot_workspaces::boot_workspaces;
pub use enable::{ext_disable, ext_enable};
pub use error::ExtError;
pub(crate) use install_dir::{native_install_dir, write_executable};
pub use list::ext_list;
pub use publish::ext_publish;
pub use reconcile::{reconcile, ReconcileAction, ReconcilePlan};
pub use row::ExtRow;
pub use start::ext_start;
pub use tool::call_ext_tool;
pub use uninstall::ext_uninstall;
