//! The **codex-family** wrapper — drives any Codex-compatible agent's `exec --json` stream. Two agents
//! share this one shim because they share the wire:
//!   * **OpenAI Codex** (`codex`), and
//!   * **Open Interpreter** (`interpreter`) — an Apache-2.0 Rust *fork of Codex* ("a coding agent for
//!     low-cost models"), so its `exec --json` and ACP surfaces are Codex-shaped.
//! This is the seam paying off: a structurally different *family* from vtcode, yet **one shim covers a
//! whole family** and a new family member is just a new `AgentProfile` (different binary), not new code.
//! Both are **FUTURE** integration targets — neither is driven against a real binary here yet; the
//! reference, exercised agent is [`super::vtcode`].
//!
//! **Schema is verified against real source** (`openinterpreter/codex-rs/exec/src/exec_events.rs`),
//! unlike the earlier best-effort guess: codex emits top-level `ThreadEvent`s (`thread.started`,
//! `turn.started/completed/failed`, `item.started/updated/completed`, `error`); the `item.*` events
//! carry a `ThreadItem { id, <flattened details tagged by type> }` whose details are `agent_message`,
//! `reasoning`, `command_execution`, `mcp_tool_call`, `file_change`, `web_search`, `todo_list`,
//! `error`, `collab_tool_call`. Tolerant: unknown event/item types decode to their `Other` arm and
//! project to nothing.

use lb_run_events::{RunEvent, RunOutcome};
use serde::Deserialize;

use crate::profile::AgentProfile;
use crate::wrapper::{AgentWrapper, Decoded};

/// The codex-family wrapper (zero-sized strategy). Used by both the `codex` and `interpreter` profiles.
#[derive(Debug, Default, Clone, Copy)]
pub struct CodexWrapper;

impl AgentWrapper for CodexWrapper {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn command_args(&self, profile: &AgentProfile, goal: &str, workspace: &str) -> Vec<String> {
        // `codex exec --json --skip-git-repo-check -C <workspace> -m <model> <goal>`
        // Auth + provider are codex's own config/login, so (unlike vtcode) no `--api-key-env`.
        vec![
            "exec".into(),
            "--json".into(),
            "--skip-git-repo-check".into(),
            "-C".into(),
            workspace.into(),
            "-m".into(),
            profile.model.model.clone(),
            goal.into(),
        ]
    }

    fn decode_line(&self, line: &str, turn: u32) -> Decoded {
        let ev: ThreadEvent = match serde_json::from_str(line) {
            Ok(ev) => ev,
            Err(_) => return Decoded::Ignore,
        };
        match ev {
            // A completed assistant message is a step boundary (driver bumps the turn).
            ThreadEvent::ItemCompleted { item }
                if matches!(item.details, Details::AgentMessage { .. }) =>
            {
                Decoded::Message(item.project(turn))
            }
            ThreadEvent::ItemStarted { item } | ThreadEvent::ItemCompleted { item } => {
                events_or_ignore(item.project(turn))
            }
            ThreadEvent::TurnCompleted => events_or_ignore(vec![RunEvent::RunFinish {
                outcome: RunOutcome::Done,
                answer: String::new(),
            }]),
            ThreadEvent::TurnFailed { error } | ThreadEvent::Error { error } => {
                events_or_ignore(vec![RunEvent::RunFinish {
                    outcome: RunOutcome::Failed,
                    answer: error.message,
                }])
            }
            // thread.started, turn.started, item.updated, unknown → nothing observable.
            _ => Decoded::Ignore,
        }
    }
}

fn events_or_ignore(events: Vec<RunEvent>) -> Decoded {
    if events.is_empty() {
        Decoded::Ignore
    } else {
        Decoded::Events(events)
    }
}

/// Top-level JSONL event from `codex exec --json` (`ThreadEvent` in codex source). Tolerant: an
/// unmodelled `type` lands in [`ThreadEvent::Other`].
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type")]
enum ThreadEvent {
    #[serde(rename = "item.started")]
    ItemStarted { item: ThreadItem },
    #[serde(rename = "item.completed")]
    ItemCompleted { item: ThreadItem },
    #[serde(rename = "turn.completed")]
    TurnCompleted,
    #[serde(rename = "turn.failed")]
    TurnFailed { error: ThreadError },
    #[serde(rename = "error")]
    Error { error: ThreadError },
    #[serde(other)]
    Other,
}

/// A fatal/turn error payload (`ThreadErrorEvent { message }`). `turn.failed` nests it under `error`;
/// the top-level `error` event carries `message` directly — both deserialise here via the alias.
#[derive(Debug, Clone, PartialEq, Deserialize)]
struct ThreadError {
    #[serde(default, alias = "message")]
    message: String,
}

/// `ThreadItem { id, <flattened details> }`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
struct ThreadItem {
    #[serde(default)]
    id: String,
    #[serde(flatten)]
    details: Details,
}

/// The item payload, internally tagged by `type` (snake_case), per `ThreadItemDetails`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Details {
    AgentMessage {
        #[serde(default)]
        text: String,
    },
    Reasoning {
        #[serde(default)]
        text: String,
    },
    CommandExecution {
        #[serde(default)]
        command: String,
        #[serde(default)]
        exit_code: Option<i32>,
        #[serde(default)]
        aggregated_output: String,
    },
    McpToolCall {
        #[serde(default)]
        server: String,
        #[serde(default)]
        tool: String,
        #[serde(default)]
        error: Option<serde_json::Value>,
    },
    /// A **non-fatal** error surfaced as an item (`ErrorItem`) — distinct from the fatal top-level
    /// `error` ThreadEvent. Seen live, e.g. "Model metadata for `glm-4.6` not found …". Surfaced as a
    /// failed tool-result so it is visible in the transcript, not silently dropped.
    Error {
        #[serde(default)]
        message: String,
    },
    #[serde(other)]
    Other,
}

impl ThreadItem {
    /// Project one item onto `RunEvent`s. Tool items emit start (on `item.started`) and result (on
    /// `item.completed`) — the driver dedups by tool-call id; emitting both keys off the same `id`.
    fn project(&self, turn: u32) -> Vec<RunEvent> {
        match &self.details {
            Details::AgentMessage { text } if !text.is_empty() => vec![RunEvent::TextDelta {
                turn,
                text: text.clone(),
            }],
            Details::Reasoning { text } if !text.is_empty() => vec![RunEvent::ReasoningDelta {
                turn,
                text: text.clone(),
            }],
            Details::CommandExecution {
                command,
                exit_code,
                aggregated_output,
            } => match exit_code {
                // exit_code present ⇒ the command finished (item.completed) → a result.
                Some(code) => vec![RunEvent::ToolCallResult {
                    id: self.id.clone(),
                    ok: (*code == 0).then(|| aggregated_output.clone()),
                    err: (*code != 0).then(|| format!("exit code {code}")),
                }],
                // No exit_code yet ⇒ the command just started → a start.
                None => vec![RunEvent::ToolCallStart {
                    id: self.id.clone(),
                    name: format!("exec: {command}"),
                }],
            },
            Details::McpToolCall {
                server,
                tool,
                error,
            } => {
                // An mcp item with an error is a failed result; otherwise treat as a start.
                if let Some(err) = error {
                    vec![RunEvent::ToolCallResult {
                        id: self.id.clone(),
                        ok: None,
                        err: Some(err.to_string()),
                    }]
                } else {
                    vec![RunEvent::ToolCallStart {
                        id: self.id.clone(),
                        name: format!("{server}/{tool}"),
                    }]
                }
            }
            Details::Error { message } if !message.is_empty() => vec![RunEvent::ToolCallResult {
                id: self.id.clone(),
                ok: None,
                err: Some(message.clone()),
            }],
            _ => vec![],
        }
    }
}
