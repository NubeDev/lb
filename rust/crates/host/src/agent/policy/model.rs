//! The **permission policy** record — the per-workspace Allow/Deny/Ask rule list the agent loop
//! consults *in front of* `caps::check` before each tool dispatch (agent-run scope Part 2,
//! "Permission policy → a standalone ws-scoped record").
//!
//! Why a standalone record and not the capability grammar: Allow/Deny/Ask is a *runtime policy over
//! tool calls* (matched on the proposed name + args at dispatch time), not a static capability.
//! Folding it into `caps` would distort the static grant model (Resolved decisions). It is **one
//! record per workspace** (`agent_policy:{ws}`), a rule list, edited by an admin cap. The policy is
//! **defense in depth**: an `Allow` here still hits `caps::check` afterwards; it only *adds* gating.
//!
//! **Deliberately small surface** (scope, "resist regex/JSONPath until a real caller needs it"):
//!   - the tool match is a single `*`-wildcard glob on the qualified tool name (no full glob crate
//!     for a one-character wildcard — a tiny matcher in `glob.rs`);
//!   - the optional arg match is a **shallow** top-level key equality (`{"path":"cmd","equals":...}`),
//!     stringly-compared against the parsed args — no nested paths, no JSONPath.
//!
//! The evaluator is a **pure function** (`tool name + parsed args + policy → Effect`), unit-testable
//! with no store (see `evaluate.rs`). Per-run overrides are deferred (Resolved decisions); the
//! evaluation is structured so a per-run layer can sit above the ws record later without changing the
//! gate.

use serde::{Deserialize, Serialize};

/// The fixed table the one-per-workspace policy record lives in. The record id is the workspace id
/// itself (`agent_policy:{ws}`) — one record per ws, addressed deterministically so the loop and the
/// `agent.policy.set` verb agree without a lookup.
pub const POLICY_TABLE: &str = "agent_policy";

/// The decision a matched rule produces — what the loop does with a proposed call **before**
/// dispatching it. Default (no rule matches) is [`Effect::Allow`]: the policy only *adds* gating, so
/// a workspace with no policy behaves exactly as before Part 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Effect {
    /// Dispatch the call as today (still subject to `caps::check`).
    Allow,
    /// Refuse the call without dispatching; the model is fed a "denied by policy" tool result.
    Deny,
    /// Suspend the run for a human decision (a durable `agent_decision`, Part 2).
    Ask,
}

/// An optional **shallow** argument match on a rule: the parsed args' top-level key `path` must equal
/// the string `equals`. No nested paths (no `a.b.c`), no regex — the deliberately small surface
/// (scope). The compare is stringly: a JSON value at `path` is rendered to its string form and
/// compared to `equals`, so `{"path":"cmd","equals":"rm -rf"}` matches `{"cmd":"rm -rf"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArgMatch {
    /// The top-level key to read from the proposed call's parsed args.
    pub path: String,
    /// The exact string the value at `path` must equal for the rule to match.
    pub equals: String,
}

/// One policy rule: a `*`-glob on the qualified tool name, an OPTIONAL shallow arg match, and the
/// [`Effect`] to apply when both match. A rule with no `arg` matches on the tool name alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    /// A `*`-wildcard glob on the qualified tool name (e.g. `shell.*`, `*`, `hello.echo`).
    pub tool: String,
    /// An optional shallow top-level arg equality the rule additionally requires.
    #[serde(default)]
    pub arg: Option<ArgMatch>,
    /// What to do when this rule matches.
    pub effect: Effect,
}

/// The workspace's policy: an ordered rule list. Order is **not** precedence — the evaluator applies
/// a fixed precedence (Deny beats Allow beats Ask) across *all* matching rules, so a later rule never
/// silently overrides an earlier Deny. An empty list = default-allow everything.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    pub rules: Vec<Rule>,
}
