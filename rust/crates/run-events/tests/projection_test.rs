//! agent-run scope Part 1 (review point 5): the live stream and a `session/load` replay yield the
//! **same** `RunEvent` view, because they are the same projection of the same durable transcript.
//! Pure data — no node/bus, a plain test.

use lb_jobs::{Job, JobStatus, Step, SuspensionDecision, TranscriptEvent};
use lb_run_events::{project, project_one, RunEvent, RunOutcome};

fn job_with(events: Vec<TranscriptEvent>, status: JobStatus) -> Job {
    let mut job = Job::new("j1", "agent-session", "the goal", 1);
    job.steps = events
        .into_iter()
        .enumerate()
        .map(|(i, event)| Step {
            index: i as u32,
            event,
        })
        .collect();
    job.cursor = job.steps.len() as u32;
    job.status = status;
    job
}

fn sample_events() -> Vec<TranscriptEvent> {
    vec![
        TranscriptEvent::AssistantTurn {
            content: "I'll look it up.".into(),
        },
        TranscriptEvent::ToolCallProposed {
            id: "c0".into(),
            name: "hello.echo".into(),
            args: r#"{"msg":"hi"}"#.into(),
        },
        TranscriptEvent::ToolResult {
            id: "c0".into(),
            ok: Some("hi".into()),
            err: None,
        },
        TranscriptEvent::SkillActivated {
            id: "repo-conventions".into(),
        },
        TranscriptEvent::AssistantTurn {
            content: "done".into(),
        },
    ]
}

#[test]
fn snapshot_and_live_replay_yield_the_same_view() {
    let job = job_with(sample_events(), JobStatus::Done);

    // The snapshot: a late watcher's catch-up over the whole transcript.
    let snapshot = project(&job);

    // The "live" view: RunStart, then each transcript event projected as it was appended, then
    // RunFinish — exactly what the loop emits incrementally.
    let mut live = vec![RunEvent::RunStart {
        goal: "the goal".into(),
    }];
    let mut turn = 0u32;
    for ev in job.events() {
        live.extend(project_one(ev, turn));
        if matches!(ev, TranscriptEvent::AssistantTurn { .. }) {
            turn += 1;
        }
    }
    live.push(RunEvent::RunFinish {
        outcome: RunOutcome::Done,
        answer: "done".into(),
    });

    assert_eq!(snapshot, live, "live deltas and a replay must be identical");
}

#[test]
fn args_deltas_and_tool_calls_are_present_from_day_one() {
    let job = job_with(sample_events(), JobStatus::Running);
    let events = project(&job);
    assert!(events
        .iter()
        .any(|e| matches!(e, RunEvent::ToolCallStart { name, .. } if name == "hello.echo")));
    assert!(events
        .iter()
        .any(|e| matches!(e, RunEvent::ToolCallArgsDelta { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, RunEvent::SkillActivated { id } if id == "repo-conventions")));
    // A running (non-terminal) run has NO RunFinish.
    assert!(!events
        .iter()
        .any(|e| matches!(e, RunEvent::RunFinish { .. })));
}

#[test]
fn a_suspended_run_projects_suspended_then_a_suspended_finish() {
    let job = job_with(
        vec![
            TranscriptEvent::AssistantTurn {
                content: "I want to run a risky tool".into(),
            },
            TranscriptEvent::ToolCallProposed {
                id: "c1".into(),
                name: "shell.run".into(),
                args: r#"{"cmd":"rm -rf x"}"#.into(),
            },
            TranscriptEvent::SuspensionOpened {
                tool_call_id: "c1".into(),
                decision_id: "agent_decision:j1:c1".into(),
            },
        ],
        JobStatus::Suspended,
    );
    let events = project(&job);
    assert!(events
        .iter()
        .any(|e| matches!(e, RunEvent::Suspended { tool_call_id, .. } if tool_call_id == "c1")));
    assert!(events.iter().any(|e| matches!(
        e,
        RunEvent::RunFinish {
            outcome: RunOutcome::Suspended,
            ..
        }
    )));
}

#[test]
fn settled_suspension_projects_a_settled_event() {
    let job = job_with(
        vec![TranscriptEvent::SuspensionSettled {
            decision_id: "agent_decision:j1:c1".into(),
            decision: SuspensionDecision::Deny,
        }],
        JobStatus::Running,
    );
    let events = project(&job);
    assert!(events.iter().any(
        |e| matches!(e, RunEvent::Settled { decision_id } if decision_id == "agent_decision:j1:c1")
    ));
}
