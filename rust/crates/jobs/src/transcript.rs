//! The typed run **transcript event** — the durable, replayable record of one thing that happened
//! in an agent run (agent-run scope Part 0, "durable, typed run state"). This replaces the job
//! step's opaque `String` (the old `Step.result`) so a suspended conversation can be **rehydrated
//! exactly** on resume — the assistant turns, the proposed tool calls *with their args*, the tool
//! results, skill activations, and any pending suspension — rather than re-derived from the goal
//! alone (the old `run.rs` behavior, which re-asked the model from scratch).
//!
//! **Versioned from day one** (scope "long-term guard"): the enum is `#[non_exhaustive]` and
//! `#[serde(tag = "kind")]`, and the job carries a `schema_version`, so a later variant or a
//! child-table migration is *additive*, never a rewrite. The transcript is the **record**; the
//! `RunEvent` stream (Part 1) is a *projection* of it — state vs motion (§3.3). Nothing here knows
//! about the bus, MCP, or a protocol; it is pure data in the lowest crate that owns the job record.
//!
//! No wall-clock and no opaque blobs: every field is a typed, deterministic value the loop sets.

use serde::{Deserialize, Serialize};

/// The current transcript schema version. Bumped only when a variant's *meaning* changes in a way
/// a reader must branch on; adding a `#[non_exhaustive]` variant does not require a bump (an old
/// reader simply ignores an unknown `kind`). Stored on the [`Job`](crate::Job) so a migration can
/// detect an older record and upconvert it.
pub const TRANSCRIPT_SCHEMA_VERSION: u32 = 1;

/// One durable thing that happened in a run, in the order it happened. Append-addressed by the
/// enclosing [`Step.index`](crate::Step): replaying step `i` upserts the same slot (idempotent
/// resume), and the loop's message/`prior`/active-skill state is *reconstructed* by folding the
/// events in index order (see `run.rs::rehydrate`).
///
/// `#[serde(tag = "kind")]` gives a stable, self-describing wire shape (`{"kind":"assistant-turn",
/// …}`); `#[non_exhaustive]` reserves the right to add variants (a token-delta record, a
/// human-authored result) without breaking a stored transcript or an external decoder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum TranscriptEvent {
    /// The model's text for a turn. `content` is what entered the conversation as the assistant
    /// message; an empty turn (calls only) is recorded with an empty string so the cursor still
    /// advances and replay is faithful.
    AssistantTurn { content: String },
    /// A tool call the model proposed this turn, captured **with its args** so an `Allow→replay`
    /// resume can re-run the *originally proposed* call from the durable record (scope resume modes).
    ToolCallProposed {
        id: String,
        name: String,
        args: String,
    },
    /// The outcome of running a proposed call, fed back to the model on the next turn. Exactly one
    /// of `ok`/`err` is set (a denial is an `err`, never a crash — agent scope deny path).
    ToolResult {
        id: String,
        ok: Option<String>,
        err: Option<String>,
    },
    /// A proposed call that will never run: the turn died (cancel, crash, detector break) before
    /// its result landed (agent-loop-hardening slice C). Recorded so the durable transcript never
    /// carries a proposed call without a resolution — a watcher's spinner resolves, and a resume
    /// folds it as a "cancelled" tool error the model can see. Additive (`#[non_exhaustive]`).
    ToolCancelled { id: String },
    /// The model activated a granted skill mid-run (Part 5). Recorded so the active-skill set
    /// survives resume — a rehydrated run re-loads exactly the skills it had activated.
    SkillActivated { id: String },
    /// A run suspended for a human decision on a specific tool call (Part 2). Carries the
    /// `agent_decision` record id the settle binds on; persisted **before** the `Suspended` event
    /// is emitted so the durable pause never trails the stream.
    SuspensionOpened {
        tool_call_id: String,
        decision_id: String,
    },
    /// A previously-opened suspension was settled (allow/deny). Recorded so a rehydrated run knows
    /// the decision already bound and resumes past it idempotently (a duplicate settle is a no-op).
    SuspensionSettled {
        decision_id: String,
        decision: SuspensionDecision,
    },
    /// A durable checkpoint a run persisted (long-running-rules-scope: `job.set`/`job.step`).
    /// `value` is a JSON-encoded string (the `ToolCallProposed.args` precedent). On resume the
    /// owner folds these — last write per `key` wins — into the run's checkpoint state, so a
    /// memoized step replays as a lookup, never a re-spend. Additive (`#[non_exhaustive]`).
    Checkpoint { key: String, value: String },
    /// An advisory progress beat (long-running-rules-scope: `job.progress`). `pct` is 0–100 when
    /// given. Observers read the latest beat; replaying one upserts its slot like any event.
    Progress { pct: Option<u32>, msg: String },
}

/// How a suspended tool call was settled. Mirrors the `agent_decision` record's decision and the
/// resume mode the loop applies (scope: ship `Deny` + `Allow`; `UseDecisionAsResult` is designed-for
/// but not built — it would be an additive `#[non_exhaustive]` variant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum SuspensionDecision {
    /// Run the originally-proposed call from the persisted args (`Allow→replay`).
    Allow,
    /// Feed the model a "denied by policy" tool result; the loop continues (the model picks a safer
    /// path).
    Deny,
}
