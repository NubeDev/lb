//! The **prefs** host service — the capability chokepoint over `lb_prefs` (prefs scope). Gated
//! tenant verbs (`prefs.get/set/resolve/set_default`) plus the grant-free utility tier
//! (`format.*`/`convert.unit`). One verb per file in `verbs.rs`; the MCP bridge in `tool.rs`.

mod authorize;
mod catalog_authorize;
mod catalog_motion;
mod catalog_tool;
mod catalog_verbs;
mod error;
mod tool;
mod verbs;

pub use authorize::authorize_prefs;
pub use catalog_motion::catalog_changed_subject;
pub use catalog_tool::{call_catalog_tool, call_prefs_catalog_tool};
pub use catalog_verbs::{message_render, message_set_catalog, prefs_catalog, CatalogView};
pub use error::PrefsSvcError;
pub use tool::{call_format_tool, call_prefs_tool};
pub use verbs::{prefs_get, prefs_resolve, prefs_set, prefs_set_default};
