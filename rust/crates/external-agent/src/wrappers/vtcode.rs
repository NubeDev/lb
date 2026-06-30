//! The **vtcode** wrapper — the reference [`AgentWrapper`] impl. The only file that knows VT Code's
//! `exec --json` argv and its NDJSON event shape; when vtcode's CLI or schema shifts, this is the one
//! file to touch. Adding codex/pi/dirge is a sibling file, not an edit here ([`super`]).
//!
//! Tolerant by design: an unrecognised `type` deserialises to [`VtcodeEvent::Other`] and decodes to
//! [`Decoded::Ignore`] rather than failing the run — vtcode is a third party we don't control.

use lb_run_events::{RunEvent, RunOutcome};
use serde::Deserialize;

use crate::profile::AgentProfile;
use crate::wrapper::{AgentWrapper, Decoded};

/// The vtcode wrapper (zero-sized strategy).
#[derive(Debug, Default, Clone, Copy)]
pub struct VtcodeWrapper;

impl AgentWrapper for VtcodeWrapper {
    fn id(&self) -> &'static str {
        "vtcode"
    }

    fn command_args(&self, profile: &AgentProfile, goal: &str, workspace: &str) -> Vec<String> {
        // `vtcode exec --json --provider P --model M --api-key-env E <goal> <workspace>`
        vec![
            "exec".into(),
            "--json".into(),
            "--provider".into(),
            profile.model.provider.clone(),
            "--model".into(),
            profile.model.model.clone(),
            "--api-key-env".into(),
            profile.model.api_key_env.clone(),
            goal.into(),
            workspace.into(),
        ]
    }

    fn decode_line(&self, line: &str, turn: u32) -> Decoded {
        match serde_json::from_str::<VtcodeEvent>(line) {
            Ok(ev @ VtcodeEvent::Message { .. }) => Decoded::Message(ev.project(turn)),
            Ok(ev) => {
                let events = ev.project(turn);
                if events.is_empty() {
                    Decoded::Ignore
                } else {
                    Decoded::Events(events)
                }
            }
            Err(_) => Decoded::Ignore,
        }
    }
}

/// One decoded NDJSON line from `vtcode exec --json`. The discriminant is vtcode's `type` field.
/// Unknown types land in [`VtcodeEvent::Other`] so a schema addition on their side never breaks us.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VtcodeEvent {
    /// A chunk of assistant-visible text.
    Message {
        #[serde(default)]
        text: String,
    },
    /// A chunk of model reasoning/thinking, kept distinct from the answer.
    Reasoning {
        #[serde(default)]
        text: String,
    },
    /// The agent decided to call one of its tools.
    ToolCall {
        id: String,
        name: String,
        /// Arguments as a JSON value; stringified when projected (matches `ToolCallArgsDelta`).
        #[serde(default)]
        args: serde_json::Value,
    },
    /// A tool call finished. `error` present ⇒ the call failed (a capability deny shows up here too).
    ToolResult {
        id: String,
        #[serde(default)]
        result: Option<serde_json::Value>,
        #[serde(default)]
        error: Option<String>,
    },
    /// The run finished. `status` is vtcode's terminal word; `text` the final answer if any.
    Done {
        #[serde(default)]
        status: Option<String>,
        #[serde(default)]
        text: String,
    },
    /// Any vtcode event type we don't (yet) map. Preserved, not an error.
    #[serde(other)]
    Other,
}

impl VtcodeEvent {
    /// Project one vtcode event onto zero or more [`RunEvent`]s (their wire → our vocabulary). Returns
    /// an empty vec for events with no platform analogue (e.g. [`Self::Other`]).
    pub fn project(&self, turn: u32) -> Vec<RunEvent> {
        match self {
            VtcodeEvent::Message { text } if !text.is_empty() => vec![RunEvent::TextDelta {
                turn,
                text: text.clone(),
            }],
            VtcodeEvent::Reasoning { text } if !text.is_empty() => {
                vec![RunEvent::ReasoningDelta {
                    turn,
                    text: text.clone(),
                }]
            }
            VtcodeEvent::ToolCall { id, name, args } => vec![
                RunEvent::ToolCallStart {
                    id: id.clone(),
                    name: name.clone(),
                },
                RunEvent::ToolCallArgsDelta {
                    id: id.clone(),
                    args: args.to_string(),
                },
            ],
            VtcodeEvent::ToolResult { id, result, error } => vec![RunEvent::ToolCallResult {
                id: id.clone(),
                ok: result.as_ref().map(|v| v.to_string()),
                err: error.clone(),
            }],
            VtcodeEvent::Done { status, text } => vec![RunEvent::RunFinish {
                outcome: outcome_of(status.as_deref()),
                answer: text.clone(),
            }],
            // Empty Message/Reasoning and Other carry nothing observable.
            _ => vec![],
        }
    }
}

/// Map vtcode's terminal `status` word onto our [`RunOutcome`]. **Fail-closed on an unrecognised word:**
/// this is an untrusted third-party agent, so a status we don't understand must NOT be read as success —
/// it maps to `Failed` so a human/retry looks, rather than silently claiming the run finished cleanly.
/// An *absent* status (`None`) is vtcode's normal "done event without a status field" and maps to
/// `Done`. (Authoritative outcome ultimately comes from the run job / process exit in #5, not this
/// self-reported word — this mapping is the motion-side hint, kept conservative.)
fn outcome_of(status: Option<&str>) -> RunOutcome {
    match status {
        None => RunOutcome::Done,
        Some("done") | Some("success") | Some("completed") | Some("ok") => RunOutcome::Done,
        Some("cancelled") | Some("canceled") => RunOutcome::Cancelled,
        Some("suspended") => RunOutcome::Suspended,
        // "error"/"failed" and anything unrecognised → fail-closed.
        Some(_) => RunOutcome::Failed,
    }
}
