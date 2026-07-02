//! Decode + projection tests for the per-agent wrappers. These need **no** model and **no**
//! subprocess: they feed real event-shaped lines through each wrapper's `decode_line` (the unit
//! boundary). Rule 9 compliant — a parse boundary, not a fake backend. The vtcode block and the codex
//! block are deliberately parallel: same assertions, different wrapper, which *is* the swap proof.

use lb_external_agent::wrapper::{AgentWrapper, Decoded};
use lb_external_agent::wrappers::vtcode::VtcodeEvent;
use lb_external_agent::{CodexWrapper, VtcodeWrapper};
use lb_run_events::{RunEvent, RunOutcome};

// ---- vtcode: the VtcodeEvent projection (their wire -> our RunEvent) --------------------------

fn vt(line: &str) -> VtcodeEvent {
    serde_json::from_str(line).expect("decodes")
}

#[test]
fn vtcode_message_projects_to_text_delta() {
    assert_eq!(
        vt(r#"{"type":"message","text":"hello"}"#).project(0),
        vec![RunEvent::TextDelta {
            turn: 0,
            text: "hello".into()
        }]
    );
}

#[test]
fn vtcode_empty_message_projects_to_nothing() {
    assert!(vt(r#"{"type":"message","text":""}"#).project(0).is_empty());
}

#[test]
fn vtcode_reasoning_projects_to_reasoning_delta() {
    assert_eq!(
        vt(r#"{"type":"reasoning","text":"thinking"}"#).project(2),
        vec![RunEvent::ReasoningDelta {
            turn: 2,
            text: "thinking".into()
        }]
    );
}

#[test]
fn vtcode_tool_call_projects_to_start_plus_args() {
    let out = vt(r#"{"type":"tool_call","id":"t1","name":"read_file","args":{"path":"a.rs"}}"#)
        .project(0);
    assert_eq!(
        out[0],
        RunEvent::ToolCallStart {
            id: "t1".into(),
            name: "read_file".into()
        }
    );
    match &out[1] {
        RunEvent::ToolCallArgsDelta { id, args } => {
            assert_eq!(id, "t1");
            assert!(args.contains("a.rs"));
        }
        other => panic!("expected args delta, got {other:?}"),
    }
}

#[test]
fn vtcode_tool_result_error_maps_to_err() {
    // A capability deny surfaces here as an error string — the topic's "deny is a tool error, not a
    // crash" rule, observed at the projection boundary.
    assert_eq!(
        vt(r#"{"type":"tool_result","id":"t1","error":"capability denied"}"#).project(0),
        vec![RunEvent::ToolCallResult {
            id: "t1".into(),
            ok: None,
            err: Some("capability denied".into()),
        }]
    );
}

#[test]
fn vtcode_done_status_maps_outcomes() {
    let cases = [
        (r#"{"type":"done","status":"error"}"#, RunOutcome::Failed),
        (
            r#"{"type":"done","status":"cancelled"}"#,
            RunOutcome::Cancelled,
        ),
        (r#"{"type":"done","status":"success"}"#, RunOutcome::Done),
        // Absent status is vtcode's normal completion → Done.
        (r#"{"type":"done","text":"final"}"#, RunOutcome::Done),
        // Fail-closed: an unrecognised status word must NOT read as success.
        (
            r#"{"type":"done","status":"weird_unknown_word"}"#,
            RunOutcome::Failed,
        ),
    ];
    for (line, want) in cases {
        match &vt(line).project(0)[0] {
            RunEvent::RunFinish { outcome, .. } => assert_eq!(*outcome, want, "for {line}"),
            other => panic!("got {other:?}"),
        }
    }
}

#[test]
fn vtcode_unknown_type_is_tolerated() {
    let ev = vt(r#"{"type":"some_future_thing","whatever":1}"#);
    assert_eq!(ev, VtcodeEvent::Other);
    assert!(ev.project(0).is_empty());
}

// ---- the AgentWrapper seam: decode_line classifies lines the same way for any agent -----------

#[test]
fn vtcode_wrapper_decode_classifies_message_as_step() {
    match VtcodeWrapper.decode_line(r#"{"type":"message","text":"hi"}"#, 0) {
        Decoded::Message(evs) => assert_eq!(
            evs,
            vec![RunEvent::TextDelta {
                turn: 0,
                text: "hi".into()
            }]
        ),
        other => panic!("expected Message, got {other:?}"),
    }
    assert_eq!(VtcodeWrapper.decode_line("not json", 0), Decoded::Ignore);
    assert_eq!(VtcodeWrapper.id(), "vtcode");
}

// Codex-family lines, in the REAL `ThreadEvent`/`ThreadItem` schema verified against
// openinterpreter/codex-rs/exec/src/exec_events.rs. The same wrapper drives Codex AND Open Interpreter.
#[test]
fn codex_wrapper_decodes_the_real_thread_event_schema() {
    // A completed agent_message item is a step boundary → Message.
    match CodexWrapper.decode_line(
        r#"{"type":"item.completed","item":{"id":"i1","type":"agent_message","text":"hi"}}"#,
        3,
    ) {
        Decoded::Message(evs) => assert_eq!(
            evs,
            vec![RunEvent::TextDelta {
                turn: 3,
                text: "hi".into()
            }]
        ),
        other => panic!("expected Message, got {other:?}"),
    }
    // A failed command_execution (exit_code != 0) → tool-result error.
    match CodexWrapper.decode_line(
        r#"{"type":"item.completed","item":{"id":"c1","type":"command_execution","command":"ls","exit_code":1,"aggregated_output":""}}"#,
        0,
    ) {
        Decoded::Events(evs) => match &evs[0] {
            RunEvent::ToolCallResult { id, err, .. } => {
                assert_eq!(id, "c1");
                assert!(err.as_ref().unwrap().contains("exit code 1"));
            }
            other => panic!("got {other:?}"),
        },
        other => panic!("expected Events, got {other:?}"),
    }
    // turn.completed → finish Done; error → finish Failed; thread.started / unknown / non-json → ignore.
    match CodexWrapper.decode_line(r#"{"type":"turn.completed","usage":{}}"#, 0) {
        Decoded::Events(evs) => assert!(matches!(
            evs[0],
            RunEvent::RunFinish {
                outcome: RunOutcome::Done,
                ..
            }
        )),
        other => panic!("expected Events, got {other:?}"),
    }
    match CodexWrapper.decode_line(r#"{"type":"error","error":{"message":"boom"}}"#, 0) {
        Decoded::Events(evs) => assert!(matches!(
            evs[0],
            RunEvent::RunFinish {
                outcome: RunOutcome::Failed,
                ..
            }
        )),
        other => panic!("expected Events, got {other:?}"),
    }
    assert_eq!(
        CodexWrapper.decode_line(r#"{"type":"thread.started","thread_id":"t1"}"#, 0),
        Decoded::Ignore
    );
    assert_eq!(CodexWrapper.decode_line("not json", 0), Decoded::Ignore);
    assert_eq!(CodexWrapper.id(), "codex");
}

// Real lines captured from `interpreter exec --json` driving Z.AI GLM-4.6 (Open Interpreter 0.0.17).
// This is the actual wire, not a guess — the wrapper must project this exact stream.
#[test]
fn codex_wrapper_projects_a_real_open_interpreter_stream() {
    let stream = [
        r#"{"type":"thread.started","thread_id":"019f188c-ce8a-7dc1-8838-8d5ce651ecf8"}"#,
        r#"{"type":"item.completed","item":{"id":"item_0","type":"error","message":"Model metadata for `glm-4.6` not found. Defaulting to fallback metadata; this can degrade performance and cause issues."}}"#,
        r#"{"type":"turn.started"}"#,
        r#"{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"PONG"}}"#,
        r#"{"type":"turn.completed","usage":{"input_tokens":10750,"cached_input_tokens":0,"output_tokens":5,"reasoning_output_tokens":0}}"#,
    ];
    let decoded: Vec<Decoded> = stream
        .iter()
        .map(|l| CodexWrapper.decode_line(l, 0))
        .collect();
    // thread.started → Ignore; the non-fatal error item → surfaced as a failed tool-result (not dropped).
    assert_eq!(decoded[0], Decoded::Ignore);
    match &decoded[1] {
        Decoded::Events(evs) => match &evs[0] {
            RunEvent::ToolCallResult { err, .. } => {
                assert!(err.as_ref().unwrap().contains("Model metadata"))
            }
            other => panic!("got {other:?}"),
        },
        other => panic!("expected Events for error item, got {other:?}"),
    }
    assert_eq!(decoded[2], Decoded::Ignore); // turn.started
                                             // the assistant answer is a step boundary carrying the PONG text.
    match &decoded[3] {
        Decoded::Message(evs) => assert_eq!(
            evs,
            &vec![RunEvent::TextDelta {
                turn: 0,
                text: "PONG".into()
            }]
        ),
        other => panic!("expected Message, got {other:?}"),
    }
    // turn.completed → finish Done.
    match &decoded[4] {
        Decoded::Events(evs) => assert!(matches!(
            evs[0],
            RunEvent::RunFinish {
                outcome: RunOutcome::Done,
                ..
            }
        )),
        other => panic!("got {other:?}"),
    }
}

// The headline swap: Open Interpreter (a Codex fork) reuses the SAME wrapper — only the profile binary
// differs. Zero new code for a whole second agent.
#[test]
fn open_interpreter_reuses_codex_wrapper_only_the_binary_differs() {
    use lb_external_agent::{AgentProfile, ModelEndpoint};
    let model = ModelEndpoint {
        provider: "openai".into(),
        model: "gpt-5.4-mini".into(),
        api_key_env: "OPENAI_API_KEY".into(),
        base_url: None,
    };
    let codex = AgentProfile::codex_default(model.clone());
    let oi = AgentProfile::open_interpreter_default(model);
    assert_eq!(codex.binary, "codex");
    assert_eq!(oi.binary, "interpreter");
    // Same decode path proves the wire is shared: both drive through CodexWrapper unchanged.
    let line = r#"{"type":"item.completed","item":{"id":"i1","type":"agent_message","text":"x"}}"#;
    assert!(matches!(
        CodexWrapper.decode_line(line, 0),
        Decoded::Message(_)
    ));
}
