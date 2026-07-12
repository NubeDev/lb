//! Slice C's orphan detection (agent-loop-hardening): a proposed call with no `ToolResult`, no
//! `ToolCancelled`, and no parking `SuspensionOpened` is an orphan; everything resolved is not.
//! Pure over the transcript vocabulary — the heal (append-at-cursor, never renumber) is proven at
//! the loop level in `lb-host`'s `agent_dangling_test`.

use lb_jobs::{orphaned_calls, SuspensionDecision, TranscriptEvent};

fn proposed(id: &str) -> TranscriptEvent {
    TranscriptEvent::ToolCallProposed {
        id: id.into(),
        name: format!("tool.{id}"),
        args: "{}".into(),
    }
}

#[test]
fn only_unresolved_proposals_are_orphans() {
    let events = vec![
        TranscriptEvent::AssistantTurn {
            content: "working".into(),
        },
        proposed("ok-call"),
        TranscriptEvent::ToolResult {
            id: "ok-call".into(),
            ok: Some("fine".into()),
            err: None,
        },
        proposed("err-call"),
        TranscriptEvent::ToolResult {
            id: "err-call".into(),
            ok: None,
            err: Some("denied".into()),
        },
        proposed("already-cancelled"),
        TranscriptEvent::ToolCancelled {
            id: "already-cancelled".into(),
        },
        proposed("parked"),
        TranscriptEvent::SuspensionOpened {
            tool_call_id: "parked".into(),
            decision_id: "job:parked".into(),
        },
        proposed("dangling-1"),
        proposed("dangling-2"),
    ];
    let refs: Vec<&TranscriptEvent> = events.iter().collect();

    let orphans = orphaned_calls(&refs);
    let ids: Vec<&str> = orphans.iter().map(|o| o.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["dangling-1", "dangling-2"],
        "resolved, cancelled, and suspension-parked calls are not orphans"
    );
    assert_eq!(orphans[0].name, "tool.dangling-1");
}

#[test]
fn a_settled_suspension_is_still_not_an_orphan() {
    // A parked call whose decision settled (result appended by the resume path) stays resolved;
    // even a parked call with no settle yet is not an orphan — the decision path owns it.
    let events = vec![
        proposed("gated"),
        TranscriptEvent::SuspensionOpened {
            tool_call_id: "gated".into(),
            decision_id: "job:gated".into(),
        },
        TranscriptEvent::SuspensionSettled {
            decision_id: "job:gated".into(),
            decision: SuspensionDecision::Deny,
        },
    ];
    let refs: Vec<&TranscriptEvent> = events.iter().collect();
    assert!(orphaned_calls(&refs).is_empty());
}

#[test]
fn an_empty_or_clean_transcript_has_no_orphans() {
    assert!(orphaned_calls(&[]).is_empty());
    let events = vec![TranscriptEvent::AssistantTurn {
        content: "just text".into(),
    }];
    let refs: Vec<&TranscriptEvent> = events.iter().collect();
    assert!(orphaned_calls(&refs).is_empty());
}
