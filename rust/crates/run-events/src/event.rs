//! The one internal **run-event vocabulary** (agent-run scope Part 1) — the single contract every
//! protocol and UI reads. It is the symmetric dual of how `caps` is the one scope model projected
//! onto store + bus + MCP: `RunEvent` is the one run model projected onto SSE, ACP `session/update`,
//! and (later) AI-SDK — each a thin `RunEvent -> wire` encoder in its own role crate.
//!
//! **Derived from the durable transcript, not emitted beside it** (the fix for review point 5). The
//! [`project`](crate::project) function turns a [`TranscriptEvent`] log into a `RunEvent` sequence;
//! a live stream and a reconnect/`session/load` replay are *the same projection*, so they can never
//! drift. The transcript is the record; these events are motion (§3.3).
//!
//! Designed for the streaming end-state from day one (scope resolved decisions): `TextDelta` and
//! explicit tool-call **argument deltas** (`ToolCallArgsDelta`) exist now even though the v1 loop
//! emits per-step (one whole-content `TextDelta` per turn) until `ModelAccess` grows a streaming
//! `turn` — at which point the loop forwards token deltas with **no change to this enum**. AI-SDK
//! (the planned next encoder) needs the arg deltas, so they are present now and the second encoder
//! is purely additive.

use serde::{Deserialize, Serialize};

/// One observable thing in a run — a projection of the durable transcript. `#[serde(tag = "type")]`
/// gives a stable, self-describing wire shape; `#[non_exhaustive]` reserves room for new variants
/// (a reasoning-delta split, a usage event) without breaking an existing encoder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum RunEvent {
    /// The run started (or a watcher attached at the very beginning). Carries the goal so a late
    /// watcher's snapshot is self-contained.
    RunStart { goal: String },
    /// A new model turn began — a step boundary. `turn` is the 0-based turn number.
    StepStart { turn: u32 },
    /// A chunk of assistant text. In v1 the loop emits one delta carrying the whole turn's content;
    /// when the gateway streams, the loop forwards many small deltas — same variant either way.
    TextDelta { turn: u32, text: String },
    /// A chunk of model *reasoning* text (kept distinct from the answer so an encoder can render it
    /// separately — e.g. ACP's `thought` updates). Emitted per-step in v1; per-token when streaming.
    ReasoningDelta { turn: u32, text: String },
    /// The model proposed a tool call — the args are known up front (the transcript records them).
    ToolCallStart { id: String, name: String },
    /// A chunk of a tool call's arguments (for AI-SDK, which streams the JSON args). v1 emits the
    /// whole args as one delta right after `ToolCallStart`; the streaming end-state emits many.
    ToolCallArgsDelta { id: String, args: String },
    /// A tool call finished — `ok`/`err` mirror the transcript's `ToolResult` (a denial is an `err`).
    ToolCallResult {
        id: String,
        ok: Option<String>,
        err: Option<String>,
    },
    /// A proposed call was cancelled before it ran (the turn died — cancel, crash heal, detector
    /// break; agent-loop-hardening slice C). A watcher resolves the call's spinner on this exactly
    /// as it would on a result — "tool running…" never hangs.
    ToolCancelled { id: String },
    /// The model activated a granted skill mid-run (Part 5).
    SkillActivated { id: String },
    /// The run suspended for a human decision on `tool_call_id`; `decision_id` is the
    /// `agent_decision` record the settle binds on (Part 2). A lifecycle client maps this to ACP
    /// `session/request_permission`; the durable pause outlives any connection.
    Suspended {
        tool_call_id: String,
        decision_id: String,
    },
    /// A previously-opened suspension settled (allow/deny) — a watcher learns the decision bound.
    Settled { decision_id: String },
    /// The run reached a terminal outcome. `outcome` is the run's status word (done/failed/
    /// cancelled/suspended); `answer` is the final assistant content if any.
    RunFinish { outcome: RunOutcome, answer: String },
}

/// The terminal word a [`RunEvent::RunFinish`] carries — the projection of the job's terminal
/// [`JobStatus`](lb_jobs::JobStatus). Distinct from a transcript event: it summarizes *how the run
/// ended* for an encoder's stop-reason mapping (ACP `StopReason`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum RunOutcome {
    /// The loop finished normally.
    Done,
    /// The loop ended in an unrecoverable error.
    Failed,
    /// The run is paused on a durable decision (terminal for the turn, restartable).
    Suspended,
    /// The run was cancelled.
    Cancelled,
}
