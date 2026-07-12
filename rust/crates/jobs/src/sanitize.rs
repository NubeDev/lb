//! Find the **orphaned tool calls** in a transcript — proposed, but with no resolution (no
//! `ToolResult`, no `ToolCancelled`, no open `SuspensionOpened` parking it for a human decision).
//! These are the dangling calls a turn that died mid-flight leaves behind (agent-loop-hardening
//! slice C): a watcher's spinner never resolves and, unhealed, they poison the next resume's view.
//!
//! Pure detection only — the **heal** (appending a `ToolCancelled` per orphan, at the cursor, so
//! existing step indices are NEVER renumbered; resume idempotency is a step-index lookup) is the
//! agent loop's job at load time. Renumbering here would silently break every persisted resume.

use std::collections::HashSet;

use super::transcript::TranscriptEvent;

/// One orphaned proposal: the call id and its name (carried for the cancel event's audit trail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrphanedCall {
    pub id: String,
    pub name: String,
}

/// The proposed calls in `events` with no resolution of any kind, in proposal order. A call parked
/// by a `SuspensionOpened` is NOT an orphan — it is awaiting a human decision, and the resume path
/// settles it (`resume_suspensions`).
pub fn orphaned_calls(events: &[&TranscriptEvent]) -> Vec<OrphanedCall> {
    let mut resolved: HashSet<&str> = HashSet::new();
    for e in events {
        match e {
            TranscriptEvent::ToolResult { id, .. } | TranscriptEvent::ToolCancelled { id } => {
                resolved.insert(id.as_str());
            }
            TranscriptEvent::SuspensionOpened { tool_call_id, .. } => {
                resolved.insert(tool_call_id.as_str());
            }
            _ => {}
        }
    }

    events
        .iter()
        .filter_map(|e| match e {
            TranscriptEvent::ToolCallProposed { id, name, .. }
                if !resolved.contains(id.as_str()) =>
            {
                Some(OrphanedCall {
                    id: id.clone(),
                    name: name.clone(),
                })
            }
            _ => None,
        })
        .collect()
}
