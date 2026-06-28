//! The agent **permission policy** — the per-workspace Allow/Deny/Ask gate the loop consults before
//! each tool dispatch (agent-run scope Part 2). Folder-of-verbs (FILE-LAYOUT §3):
//!   - `model`    — the [`Policy`]/[`Rule`]/[`Effect`] record shape (`agent_policy:{ws}`).
//!   - `glob`     — the tiny `*`-wildcard matcher for the tool-name match (no glob crate for 1 char).
//!   - `evaluate` — the **pure** evaluator (name+args+policy → [`Effect`], Deny>Allow>Ask).
//!   - `store`    — load/save the ws record (raw store verbs; no auth — the verb gate is upstream).
//!
//! The policy sits **in front of** `caps::check`, never replacing it (defense in depth): an `Allow`
//! here still hits the capability chokepoint inside `lb_mcp::call`. Default-allow when no rule matches
//! keeps the gate purely additive over the pre-Part-2 behavior.

mod evaluate;
mod glob;
mod model;
mod store;

pub use evaluate::evaluate;
pub use model::{ArgMatch, Effect, Policy, Rule, POLICY_TABLE};
pub use store::{load_policy, save_policy};
