//! `control-engine` — the native (Tier-2) Control Engine bridge extension (control-engine scope). A
//! thin binary over the library's control loop ([`control_engine::serve::serve`]); the modules it wires
//! live in the lib so the integration tests can drive the registry verbs / resolve against a real
//! gateway (lib+bin split, the ROS-sidecar precedent).
//!
//! A supervised OS child that holds the long-lived CE REST/WS connection (the `rubix-ce` client) and
//! serves the caps-gated `control-engine.*` MCP surface over `Content-Length`-framed stdio using the
//! SAME `lb-supervisor` wire types the host uses — so the child↔host ABI cannot drift. Stateless
//! (§3.4): the `ce_appliance` registry lives in SurrealDB (reached via the `store.*` host callback);
//! the CE client cache is a pure connection pool a kill + respawn rebuilds.
//!
//! Tools served (the NAME is the cap gate — `mcp:control-engine.<verb>:call`):
//!   - graph reads: `control-engine.tree {appliance, node?, depth?}` · `control-engine.schema {appliance}`
//!   - registry (S4): `control-engine.appliance.add|list|remove` — the `ce_appliance` CRUD.

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    control_engine::serve::serve().await;
}
