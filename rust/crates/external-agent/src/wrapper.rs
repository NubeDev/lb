//! The [`AgentWrapper`] seam — the one thing that differs between external agents, isolated so adding
//! a *future* agent (codex, pi.dev, dirge, …) is **a new impl in its own file**, never a change to the
//! driver. Only vtcode is exercised today; the seam exists so those others are accounted for without a
//! redesign. This is the topic's "swap test" in miniature (external-agent-scope umbrella exit gate:
//! "the same invoke path drives a second agent via a second profile with no code change"): the driver
//! ([`crate::driver`]) is generic over this trait, so a second wrapper is purely additive.
//!
//! A wrapper owns exactly two agent-specific facts:
//! 1. **how to launch it** — the argv that turns a profile + goal + workspace into a subprocess that
//!    streams machine-readable events on stdout;
//! 2. **how to read it** — decoding one stdout line into zero or more platform [`RunEvent`]s.
//!
//! Everything else (spawn, stdout pump, `RunStart`/`StepStart` bracketing, timeout/kill) is shared in
//! the driver and identical across agents. Each wrapper stays a thin, tolerant adapter over a third
//! party we don't control — unknown event shapes are dropped, never fatal.

use lb_run_events::RunEvent;

use crate::profile::AgentProfile;

/// One decoded stdout line's contribution to the run, as the wrapper sees it. The driver turns these
/// into the final `RunEvent` stream (adding `StepStart` boundaries and ignoring [`Decoded::Ignore`]).
/// Keeping this distinct from `RunEvent` lets a wrapper say "this line was a model message" (so the
/// driver can bump the turn counter) without each wrapper re-implementing turn numbering.
#[derive(Debug, Clone, PartialEq)]
pub enum Decoded {
    /// A model message/answer chunk — a step boundary. Carries the already-projected events.
    Message(Vec<RunEvent>),
    /// Any other projected events (reasoning, tool call/result, finish) — not a step boundary.
    Events(Vec<RunEvent>),
    /// A line with no platform analogue (banner, unknown event type, blank) — dropped.
    Ignore,
}

/// How to launch and read one family of external agent. Implemented once per agent, in its own file
/// under `wrappers/`. Stateless: an impl is a zero-sized strategy, so a node can hold a registry of
/// them and pick by [`AgentProfile`] without any per-run setup.
pub trait AgentWrapper: Send + Sync {
    /// The wrapper's id, matching the `binary`/flavour a profile selects (e.g. `"vtcode"`, `"codex"`).
    /// Lets a registry resolve `profile -> wrapper` (the seam the future `AgentRuntime` registry, #1,
    /// plugs into).
    fn id(&self) -> &'static str;

    /// Build the argv (after the binary) for a headless, JSON-streaming run of `goal` in `workspace`.
    /// The driver prepends `profile.binary` and runs it in `workspace`. The wrapper passes the API-key
    /// **env var name** (`profile.model.api_key_env`), never the key value.
    fn command_args(&self, profile: &AgentProfile, goal: &str, workspace: &str) -> Vec<String>;

    /// Decode one stdout line into its [`Decoded`] contribution. `turn` is the driver's current turn
    /// counter (wrappers are per-step like the v1 in-house loop; the driver owns numbering). A line
    /// the wrapper can't parse must return [`Decoded::Ignore`], not panic — third-party schemas drift.
    fn decode_line(&self, line: &str, turn: u32) -> Decoded;
}
