//! The `ce_appliance` registry (control-engine scope, S4): the workspace-scoped record model
//! ([`record`]) and its generic `store.*`-callback access ([`store`]). The registry VERBS live under
//! `tools/appliance/`; appliance→base resolution for the graph verbs lives in `crate::resolve`.

pub mod record;
pub mod store;
