//! Map a channel id to the two names a channel touches: its capability *resource* and its
//! bus *key expressions*. One place owns the convention so post/history/subscribe agree.
//!
//! - capability resource: `chan/{cid}` — the held grant is `bus:chan/{cid}:pub` / `:sub`
//!   (or `bus:chan/*:sub` across all channels). NO `ws/` prefix in the cap — the workspace is
//!   the request's `ws`, checked by gate 1 (auth-caps scope).
//! - bus message key (publish):   `chan/{cid}/msg/{id}` (workspace-prefixed by `lb_bus`).
//! - bus subscribe key (listen):  `chan/{cid}/msg/**`.
//!
//! Both the cap-check and the bus key are built from the same `cid` here, so they cannot
//! drift — the structural reason a cross-workspace or cross-channel listen cannot leak.

/// The capability resource for channel `cid` (matched against `bus:chan/*:…` grants).
pub fn cap_resource(cid: &str) -> String {
    format!("chan/{cid}")
}

/// The workspace-relative bus key a single message `id` publishes to.
pub fn msg_key(cid: &str, id: &str) -> String {
    format!("chan/{cid}/msg/{id}")
}

/// The workspace-relative bus key a single deleted message `id` publishes its tombstone to.
/// Distinct from [`msg_key`] because the message feed deserializes to an `Item` and would drop a
/// tombstone payload — a delete needs its own key so the stream can route it to a `delete` event.
pub fn del_key(cid: &str, id: &str) -> String {
    format!("chan/{cid}/del/{id}")
}

/// The workspace-relative bus key expression that subscribes to every message in `cid`.
pub fn sub_key(cid: &str) -> String {
    format!("chan/{cid}/msg/**")
}

/// The workspace-relative bus key expression that subscribes to every deletion in `cid`.
pub fn del_sub_key(cid: &str) -> String {
    format!("chan/{cid}/del/**")
}
