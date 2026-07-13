//! The ONE transcript-write chokepoint (agent-loop-hardening slice C). Every durable transcript
//! append in the agent service — the loop's turns/proposals/results, a suspension opening, a
//! settled decision's replay — goes through [`TranscriptWriter::append`], which (1) appends the
//! typed event at the next slot (idempotent, append-addressed — `lb_jobs::append_event`), (2)
//! publishes its `RunEvent` projection as motion (durable-before-motion, §3.3; `project_one` is
//! the SAME projection the snapshot uses so live and reconnect views can never diverge), and (3)
//! tracks which proposed calls are still unresolved.
//!
//! That tracking is the **dangling-call invariant**: a turn that dies after proposing calls calls
//! [`TranscriptWriter::cancel_pending`], so the durable record never keeps a proposal without a
//! resolution (`ToolResult`, `ToolCancelled`, or a parking `SuspensionOpened`) — the poison that
//! made a watcher hang in "tool running…" and a resume misread the gap. Before this, four separate
//! call sites appended events with hand-carried indices; consolidating them here is what makes the
//! invariant enforceable at all.

use lb_jobs::{append_event, TranscriptEvent};
use lb_run_events::project_one;

use super::error::AgentError;
use crate::boot::Node;
use crate::run_events::publish_run_event;

/// The run's transcript writer: owns the next slot index, the current turn number (for the
/// projection), and the set of proposed-but-unresolved call ids.
pub(crate) struct TranscriptWriter<'a> {
    pub(crate) node: &'a Node,
    pub(crate) ws: &'a str,
    pub(crate) job_id: &'a str,
    /// The next transcript slot to append at (the durable cursor's local mirror).
    pub(crate) index: u32,
    /// The loop's current turn number — carried into the `RunEvent` projection (`StepStart`).
    pub(crate) turn: u32,
    /// Proposed calls with no resolution yet — what `cancel_pending` must resolve on a dead turn.
    pending: Vec<String>,
}

impl<'a> TranscriptWriter<'a> {
    pub(crate) fn new(node: &'a Node, ws: &'a str, job_id: &'a str, index: u32, turn: u32) -> Self {
        Self {
            node,
            ws,
            job_id,
            index,
            turn,
            pending: Vec::new(),
        }
    }

    /// Append `event` durably at the next slot, publish its projection, advance. The durable append
    /// is the record (it must land, hence `?`); the bus publish is best-effort motion.
    pub(crate) async fn append(&mut self, event: TranscriptEvent) -> Result<(), AgentError> {
        match &event {
            TranscriptEvent::ToolCallProposed { id, .. } => self.pending.push(id.clone()),
            TranscriptEvent::ToolResult { id, .. } | TranscriptEvent::ToolCancelled { id } => {
                self.pending.retain(|p| p != id)
            }
            // A suspension PARKS the call for a human decision — resolved later by the settle path,
            // deliberately not a dangling proposal.
            TranscriptEvent::SuspensionOpened { tool_call_id, .. } => {
                self.pending.retain(|p| p != tool_call_id)
            }
            _ => {}
        }

        append_event(
            &self.node.store,
            self.ws,
            self.job_id,
            self.index,
            event.clone(),
        )
        .await?;
        self.index += 1;
        for run_event in project_one(&event, self.turn) {
            publish_run_event(&self.node.bus, self.ws, self.job_id, &run_event).await;
        }
        Ok(())
    }

    /// The LOAD-TIME HEAL (slice C's sanitizer): resolve every orphaned proposal in `events` — no
    /// result, no cancel, no parking suspension; what a killed process left behind — as
    /// `ToolCancelled`, APPENDED at the cursor. Existing step indices are NEVER renumbered (resume
    /// idempotency is a step-index lookup). Returns the heal events so the caller can fold them
    /// into the rehydrated view (the model sees "cancelled", not a silent gap). Pre-fix records
    /// heal lazily on their first resume.
    pub(crate) async fn heal_orphans(
        &mut self,
        events: &[&TranscriptEvent],
    ) -> Result<Vec<TranscriptEvent>, AgentError> {
        let healed: Vec<TranscriptEvent> = lb_jobs::orphaned_calls(events)
            .into_iter()
            .map(|o| TranscriptEvent::ToolCancelled { id: o.id })
            .collect();
        for event in &healed {
            self.append(event.clone()).await?;
        }
        Ok(healed)
    }

    /// The dead-turn protocol: resolve every still-pending proposal as `ToolCancelled` — one
    /// durable event + one `ToolCancelled` run event each (a watcher's spinner resolves; the
    /// record carries no dangling call). No-op when nothing is pending.
    pub(crate) async fn cancel_pending(&mut self) -> Result<(), AgentError> {
        for id in std::mem::take(&mut self.pending) {
            self.append(TranscriptEvent::ToolCancelled { id }).await?;
        }
        Ok(())
    }
}
