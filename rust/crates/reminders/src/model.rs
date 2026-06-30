//! The **reminder** record — a durable, workspace-scoped schedule that fires one action when it
//! comes due (reminders scope). A reminder is *state*: it lives in the store at `reminder:{id}`
//! within a workspace namespace (the hard wall, §7), so a schedule survives a crash and a node
//! restart. The reactor that drives firing is *motion* (the host `reminder` service).
//!
//! The record carries:
//!   - `schedule` — a standard 5-field cron string (the storage format; the UI never asks a human
//!     to type it). Consumed unchanged by `next_after` (croner) on the injected clock.
//!   - `max_runs` — an optional hard cap (`Some(n)`, n≥1). `None` = recurring forever. `runs` is
//!     the count of firings so far; when `runs` reaches `max_runs` the reminder is `Done`.
//!   - `enabled` — the on/off switch (pause/resume without deleting).
//!   - `next_attempt_ts` — the next instant this reminder should fire, computed from `schedule`.
//!   - `action` — the tagged union of what firing does (channel post / MCP tool / outbox effect).
//!   - `principal_sub` — the creator's identity. The reactor RE-RESOLVES its caps from the durable
//!     grant store at fire time and re-checks the action's own cap under it (reminders scope:
//!     principal capture at fire time, not create time — a revoked grant stops the firing).
//!
//! `ts` is a caller-injected logical timestamp (testing §3 — no wall-clock inside the crate).

use serde::{Deserialize, Serialize};

/// Where a reminder is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReminderStatus {
    /// `enabled` and not yet exhausted — the reactor considers it for firing.
    Active,
    /// Terminal: a one-shot fired, or `runs` reached `max_runs`. Never fired again. Kept for
    /// audit/history (a `Done` reminder is not deleted).
    Done,
}

/// The action a reminder fires. One action per reminder (chaining is the rule-chains' job; a
/// reminder MAY call a chain via the `McpTool` action). `Outbox` is the must-deliver class — the
/// effect rides the transactional outbox + relay, never raw pub/sub (the durability rule).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Action {
    /// Post a message to a channel (the inbox seam). Firing re-checks `bus:chan/{channel}:pub`
    /// under the stored principal and writes a durable `lb_inbox::Item`.
    #[serde(rename_all = "kebab-case")]
    ChannelPost { channel: String, body: String },
    /// Call an MCP tool under the stored principal. Firing re-enters the host `call_tool`
    /// chokepoint, which re-checks `mcp:{tool}:call` — authoritative validation at fire time (tool
    /// schemas evolve between create and fire). Create-time does only a best-effort schema check.
    #[serde(rename_all = "kebab-case")]
    McpTool {
        tool: String,
        #[serde(default)]
        args: serde_json::Value,
    },
    /// Emit a must-deliver effect through the transactional outbox. Firing enqueues a fresh
    /// `Effect` (id derived from `(reminder_id, scheduled_ts)`); the relay owns delivery.
    #[serde(rename_all = "kebab-case")]
    Outbox {
        target: String,
        action: String,
        #[serde(default)]
        payload: String,
    },
}

/// A durable reminder = a workspace-scoped schedule that fires one action. `id` is workspace-unique
/// and stable (re-`create`/`update` upserts the same `reminder:{id}` row).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reminder {
    pub id: String,
    /// The 5-field cron schedule (storage format). Parsed by `next_after` on the injected clock.
    pub schedule: String,
    /// `Some(n)` (n≥1) = fire at most `n` times then stop; `None` = recurring forever.
    pub max_runs: Option<u32>,
    /// How many times this reminder has fired. `runs == max_runs` ⇒ `Done`.
    pub runs: u32,
    /// The on/off switch. A `false` reminder is skipped by the scan and resumes when re-enabled.
    pub enabled: bool,
    pub status: ReminderStatus,
    pub action: Action,
    /// The creator's identity. The reactor re-resolves caps from the grant store at fire time.
    pub principal_sub: String,
    /// The next instant (logical `ts`) this reminder should fire. `0` ⇒ not yet scheduled.
    pub next_attempt_ts: u64,
    /// Soft-delete tombstone (idempotent delete + sync-friendly). Tombstoned rows never fire/list.
    pub deleted: bool,
    /// Caller-injected logical timestamp of the last write (no wall-clock — testing §3).
    pub ts: u64,
}

impl Reminder {
    /// Build a fresh active reminder. The caller sets `next_attempt_ts` via [`next_after`] (the
    /// reactor/create verb does this); this constructor leaves it `0` deliberately so the caller
    /// must choose the anchor instant explicitly.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        schedule: impl Into<String>,
        max_runs: Option<u32>,
        action: Action,
        principal_sub: impl Into<String>,
        ts: u64,
    ) -> Self {
        Self {
            id: id.into(),
            schedule: schedule.into(),
            max_runs,
            runs: 0,
            enabled: true,
            status: ReminderStatus::Active,
            action,
            principal_sub: principal_sub.into(),
            next_attempt_ts: 0,
            deleted: false,
            ts,
        }
    }
}
