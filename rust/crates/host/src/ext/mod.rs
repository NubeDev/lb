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
//! (`mcp:ext.disable:call`), `ext.uninstall` (`mcp:ext.uninstall:call`), `ext.publish`
//! (`mcp:ext.publish:call` — upload a signed artifact, verify-before-store), plus the un-gated boot
//! `reconcile` the node calls on start. The MCP bridge ([`call_ext_tool`]) exposes the gated verbs.

mod enable;
mod error;
mod list;
mod publish;
mod reconcile;
mod row;
mod tool;
mod uninstall;

pub use enable::{ext_disable, ext_enable};
pub use error::ExtError;
pub use list::ext_list;
pub use publish::ext_publish;
pub use reconcile::{reconcile, ReconcileAction, ReconcilePlan};
pub use row::ExtRow;
pub use tool::call_ext_tool;
pub use uninstall::ext_uninstall;
