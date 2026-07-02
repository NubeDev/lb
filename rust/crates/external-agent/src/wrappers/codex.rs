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
        // `codex exec --json --skip-git-repo-check --sandbox workspace-write -c approval_policy=never
        //   -C <workspace> [provider -c overrides] -m <model> <goal>`
        //
        // `--sandbox workspace-write` is load-bearing: `exec` defaults to `read-only`, so without it the
        // agent discovers it cannot write into its scratch dir mid-run and gives up ("the environment is
        // read-only") — sometimes with an empty final message, which reads as "the agent did nothing".
        // `approval_policy=never` is its headless partner: there is no human to approve a command, so any
        // other policy stalls the run waiting on a prompt that never comes. The agent is already confined
        // to the isolated scratch `workspace` (its cwd) and re-checked under our caps, so workspace-write
        // is the right blast radius — not `danger-full-access`.
        let mut args = vec![
            "exec".into(),
            "--json".into(),
            "--skip-git-repo-check".into(),
            "--sandbox".into(),
            "workspace-write".into(),
            "-c".into(),
            "approval_policy=never".into(),
            "-C".into(),
            workspace.into(),
        ];
        // When the profile names an OpenAI-compatible `base_url`, configure codex's `model_providers`
        // via `-c` overrides so the agent reaches OUR endpoint (Z.AI coding today; the gateway under
        // model-routing #4) rather than its own login. `wire_api=chat` is mandatory: codex defaults to
        // the OpenAI *Responses* API (`/responses`, 404 on Z.AI), while Z.AI — and our gateway — speak
        // Chat Completions. The provider id is the profile's `provider` (must NOT collide with a codex
        // built-in like `zai`, which points at the throttled standard endpoint). The **name** of the
        // key env var is passed, never the value (the key lives in this process's env — model-routing
        // #4 mints a scoped one). Absent `base_url`, fall back to codex's own auth/config (no overrides).
        if let Some(base_url) = &profile.model.base_url {
            let p = &profile.model.provider;
            args.extend([
                "-c".into(),
                format!("model_providers.{p}.name={p}"),
                "-c".into(),
                format!("model_providers.{p}.base_url={base_url}"),
                "-c".into(),
                format!("model_providers.{p}.env_key={}", profile.model.api_key_env),
                "-c".into(),
                format!("model_providers.{p}.wire_api=chat"),
                "-c".into(),
                format!("model_provider={p}"),
            ]);
        }
        args.extend(["-m".into(), profile.model.model.clone(), goal.into()]);
        args
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::ModelEndpoint;

    fn zai_model() -> ModelEndpoint {
        ModelEndpoint {
            provider: "zaicoding".into(),
            model: "glm-4.6".into(),
            api_key_env: "ZAI_API_KEY".into(),
            base_url: Some("https://api.z.ai/api/coding/paas/v4".into()),
        }
    }

    // With a base_url, the wrapper emits the codex `model_providers` `-c` overrides — the EXACT set
    // verified against a real Z.AI GLM-4.6 run (name/base_url/env_key/wire_api=chat + model_provider).
    // `wire_api=chat` is the load-bearing one: codex defaults to `/responses` (404 on Z.AI).
    #[test]
    fn command_args_emit_provider_overrides_when_base_url_is_set() {
        let profile = AgentProfile::open_interpreter_default(zai_model());
        let args = CodexWrapper.command_args(&profile, "hi", "/ws");
        let joined = args.join(" ");
        assert!(joined
            .contains("model_providers.zaicoding.base_url=https://api.z.ai/api/coding/paas/v4"));
        assert!(joined.contains("model_providers.zaicoding.env_key=ZAI_API_KEY"));
        assert!(joined.contains("model_providers.zaicoding.wire_api=chat"));
        assert!(joined.contains("model_provider=zaicoding"));
        // Only the env var NAME is passed (`env_key=ZAI_API_KEY`), never a key value — the wrapper
        // has no access to the secret, so no argv token can carry it. Asserted by construction: the
        // sole key-related token is the env var name.
        assert!(joined.contains("env_key=ZAI_API_KEY"));
        // Still a headless JSON exec of the goal against the model.
        assert!(joined.starts_with(
            "exec --json --skip-git-repo-check --sandbox workspace-write -c approval_policy=never -C /ws"
        ));
        // The sandbox is workspace-write (the agent must write its scratch dir), never read-only (the
        // silent-failure default) — and headless, so approvals are off.
        assert!(joined.contains("--sandbox workspace-write"));
        assert!(joined.contains("approval_policy=never"));
        assert_eq!(args.last().unwrap(), "hi");
        assert!(joined.contains("-m glm-4.6"));
    }

    // Without a base_url, the wrapper falls back to codex's own login/config — no provider overrides
    // (the pre-Z.AI behavior the earlier tests rely on).
    #[test]
    fn command_args_omit_provider_overrides_when_base_url_is_none() {
        let model = ModelEndpoint {
            provider: "openai".into(),
            model: "gpt-5.4-mini".into(),
            api_key_env: "OPENAI_API_KEY".into(),
            base_url: None,
        };
        let profile = AgentProfile::codex_default(model);
        let args = CodexWrapper.command_args(&profile, "hi", "/ws");
        assert!(!args.join(" ").contains("model_provider"));
        assert!(args.contains(&"-m".to_string()));
    }
}
