//! The cross-node tool-call wire: the bus key a routed call rides on, and the request/reply
//! envelope. One place owns the convention so the calling node's `query` and the serving
//! node's queryable cannot drift (mirrors the channel `key.rs` discipline).
//!
//! Two keys, both workspace-prefixed by `lb_bus` to `ws/{id}/…` — so a call authorized for
//! workspace B can never reach a queryable serving workspace A (the workspace wall, §7):
//!
//! - `mcp/{ext}/{node}/call` ([`node_call_key`]) — **the route**. Exactly one node declares it,
//!   so a routed call has exactly one responder by construction.
//! - `mcp/{ext}/call` ([`call_key`]) — the legacy shared key. Every host declares it, so it is a
//!   fan-in; it no longer carries calls and survives only for mixed-version fleets.
//! - request: `{ tool: "<ext>.<tool>", input: "<json>" }`.
//! - reply:   `Ok(output_json)` or `Err(message)` — the serving node's local dispatch result.

use lb_bus::NodeId;
use serde::{Deserialize, Serialize};

/// The workspace-relative bus key for routing calls to extension `ext`, on ANY node hosting it.
///
/// **This key is a fan-in and no longer carries calls** (routed-node-dispatch, #81). Every node
/// hosting `{ext}` declares it, so when a fleet runs one extension on several nodes they ALL
/// answer and `lb_bus::query` keeps whichever replied first — a silent coin flip that could
/// provision the wrong physical box. Dispatch now always uses [`node_call_key`] instead, since
/// resolve always knows the node.
///
/// It survives for two reasons: hosts still declare it so a **mixed-version fleet** works (an old
/// caller that predates #81 emits only this key), and removing a declared key is a breaking change
/// to anything already listening. Treat it as the transitional path, not the route.
pub fn call_key(ext: &str) -> String {
    format!("mcp/{ext}/call")
}

/// The workspace-relative bus key for routing a call to extension `ext` **on a specific node**.
///
/// Only that one node declares this key, which makes `lb_bus::query`'s "exactly one responder"
/// assumption true **by construction** rather than by comment. That is the whole reason the node
/// rides the KEY rather than a field inside `CallRequest` (scope, "Rejected: a node field inside
/// `CallRequest`"): a payload field would leave every host answering the shared key, receiving and
/// deserializing calls that are not theirs, with correctness depending on all N nodes implementing
/// the discard identically. Routing in the key makes the network do the addressing.
///
/// `node` is a [`NodeId`], which cannot contain a key-structural character — so it is interpolated
/// raw, with no encoding to keep in sync between here and the serving node's declaration.
pub fn node_call_key(ext: &str, node: &NodeId) -> String {
    format!("mcp/{ext}/{node}/call")
}

/// The routed call request: the qualified tool name and the JSON input, as bytes on the bus.
#[derive(Serialize, Deserialize)]
pub struct CallRequest {
    pub tool: String,
    pub input: String,
}

/// The routed call reply: the serving node's local dispatch outcome, serialized back.
#[derive(Serialize, Deserialize)]
pub enum CallReply {
    Ok(String),
    Err(String),
}
