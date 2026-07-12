//! Project a durable transcript into the [`RunEvent`] sequence (agent-run scope Part 1) — the one
//! function both a late watcher's **snapshot** and the **live replay** go through, so they yield the
//! identical view (the fix for review point 5: live and `session/load` can never drift because they
//! are the *same* projection of the *same* record).
//!
//! Pure: transcript events in, run events out — no store, no bus, no clock. The live loop emits the
//! same `RunEvent`s incrementally as it appends transcript events; a reconnecting watcher calls this
//! over the persisted transcript to rebuild state, then receives subsequent live deltas. Identical
//! input ⇒ identical output is exactly the property the unit test pins.

use lb_jobs::{Job, JobStatus, TranscriptEvent};

use crate::event::{RunEvent, RunOutcome};

/// Project a whole job into its `RunEvent` sequence — the snapshot a late watcher receives before it
/// starts seeing live deltas. Begins with `RunStart`, folds each transcript event, and (if the job
/// has reached a terminal state) ends with `RunFinish`.
pub fn project(job: &Job) -> Vec<RunEvent> {
    let mut out = vec![RunEvent::RunStart {
        goal: job.payload.clone(),
    }];

    let mut turn: u32 = 0;
    let mut last_content = String::new();

    for event in job.events() {
        match event {
            TranscriptEvent::AssistantTurn { content } => {
                out.push(RunEvent::StepStart { turn });
                if !content.is_empty() {
                    out.push(RunEvent::TextDelta {
                        turn,
                        text: content.clone(),
                    });
                    last_content = content.clone();
                }
                turn += 1;
            }
            TranscriptEvent::ToolCallProposed { id, name, args } => {
                out.push(RunEvent::ToolCallStart {
                    id: id.clone(),
                    name: name.clone(),
                });
                // v1 emits the whole args as one delta; the streaming end-state emits many.
                out.push(RunEvent::ToolCallArgsDelta {
                    id: id.clone(),
                    args: args.clone(),
                });
            }
            TranscriptEvent::ToolResult { id, ok, err } => {
                out.push(RunEvent::ToolCallResult {
                    id: id.clone(),
                    ok: ok.clone(),
                    err: err.clone(),
                });
            }
            TranscriptEvent::ToolCancelled { id } => {
                out.push(RunEvent::ToolCancelled { id: id.clone() });
            }
            TranscriptEvent::SkillActivated { id } => {
                out.push(RunEvent::SkillActivated { id: id.clone() });
            }
            TranscriptEvent::SuspensionOpened {
                tool_call_id,
                decision_id,
            } => out.push(RunEvent::Suspended {
                tool_call_id: tool_call_id.clone(),
                decision_id: decision_id.clone(),
            }),
            TranscriptEvent::SuspensionSettled { decision_id, .. } => out.push(RunEvent::Settled {
                decision_id: decision_id.clone(),
            }),
            // `#[non_exhaustive]`: an unknown future transcript variant is skipped (an old encoder
            // projects what it understands) rather than panicking.
            _ => {}
        }
    }

    if let Some(outcome) = terminal_outcome(job.status) {
        out.push(RunEvent::RunFinish {
            outcome,
            answer: last_content,
        });
    }
    out
}

/// The single transcript event's projection — what the **live loop** emits as it appends one event,
/// so a live watcher sees exactly what the snapshot would have produced for that event. `turn` is
/// the current turn number the loop tracks. Returns the (possibly several) run events for that one
/// transcript event.
pub fn project_one(event: &TranscriptEvent, turn: u32) -> Vec<RunEvent> {
    match event {
        TranscriptEvent::AssistantTurn { content } => {
            let mut v = vec![RunEvent::StepStart { turn }];
            if !content.is_empty() {
                v.push(RunEvent::TextDelta {
                    turn,
                    text: content.clone(),
                });
            }
            v
        }
        TranscriptEvent::ToolCallProposed { id, name, args } => vec![
            RunEvent::ToolCallStart {
                id: id.clone(),
                name: name.clone(),
            },
            RunEvent::ToolCallArgsDelta {
                id: id.clone(),
                args: args.clone(),
            },
        ],
        TranscriptEvent::ToolResult { id, ok, err } => vec![RunEvent::ToolCallResult {
            id: id.clone(),
            ok: ok.clone(),
            err: err.clone(),
        }],
        TranscriptEvent::ToolCancelled { id } => vec![RunEvent::ToolCancelled { id: id.clone() }],
        TranscriptEvent::SkillActivated { id } => vec![RunEvent::SkillActivated { id: id.clone() }],
        TranscriptEvent::SuspensionOpened {
            tool_call_id,
            decision_id,
        } => vec![RunEvent::Suspended {
            tool_call_id: tool_call_id.clone(),
            decision_id: decision_id.clone(),
        }],
        TranscriptEvent::SuspensionSettled { decision_id, .. } => vec![RunEvent::Settled {
            decision_id: decision_id.clone(),
        }],
        _ => vec![],
    }
}

/// Map a (possibly non-terminal) job status to a `RunFinish` outcome — `None` while the run is still
/// `Running` (no `RunFinish` is emitted for a live, unfinished run).
pub fn terminal_outcome(status: JobStatus) -> Option<RunOutcome> {
    match status {
        JobStatus::Running => None,
        JobStatus::Done => Some(RunOutcome::Done),
        JobStatus::Failed => Some(RunOutcome::Failed),
        JobStatus::Suspended => Some(RunOutcome::Suspended),
        JobStatus::Cancelled => Some(RunOutcome::Cancelled),
    }
}
