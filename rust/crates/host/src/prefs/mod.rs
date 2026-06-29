//! The **prefs** host service — the capability chokepoint over `lb_prefs` (prefs scope). Gated
//! tenant verbs (`prefs.get/set/resolve/set_default`) plus the grant-free utility tier
//! (`format.*`/`convert.unit`). One verb per file in `verbs.rs`; the MCP bridge in `tool.rs`.

mod authorize;
mod error;
mod tool;
mod verbs;

pub use authorize::authorize_prefs;
pub use error::PrefsSvcError;
pub use tool::{call_format_tool, call_prefs_tool};
pub use verbs::{prefs_get, prefs_resolve, prefs_set, prefs_set_default};
