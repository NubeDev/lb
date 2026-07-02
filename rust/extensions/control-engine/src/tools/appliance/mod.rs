//! The `control-engine.appliance.*` registry verbs (folder-of-verbs, one file per verb). These are
//! the S4 registry surface: `add` (register), `list` (read), `remove` (delete). Each self-checks its
//! own `mcp:control-engine.appliance.<verb>:call` grant (the inbound `native.call` carries no caller
//! identity) and reaches the `ce_appliance` table only through the generic `store.*` callbacks.

pub mod add;
pub mod list;
pub mod remove;
