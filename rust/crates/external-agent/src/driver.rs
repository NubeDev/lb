//! Spawn a real external-agent subprocess and stream its output as [`RunEvent`]s — **generic over the
//! [`AgentWrapper`]**, so the same code drives vtcode, codex, or any future agent. This is the shared
//! half of the driver: spawn, pump stdout, bracket the stream (`RunStart` … `StepStart` per message),
//! enforce a liveness timeout. The agent-specific half (argv + line decode) lives in the wrapper.
//!
//! Standalone core of acp-driver sub-scope #2, minus the ACP-SDK transport (the JSON-stream surface
//! needs no extra crate). The seam it presents — *given a wrapper + profile + goal, yield `RunEvent`s
//! until the run ends* — is what the eventual `AgentRuntime` trait (#1) adapts onto. **Not** wired
//! into the node: nothing in the workspace depends on this crate yet (Cargo.toml header).
//!
//! Deliberately NOT here (later sub-scopes): the capability wall / built-ins-off enforcement (#3),
//! gateway model routing (#4), the durable job / resume / supervision (#5). The timeout is a liveness
//! bound, not the #5 supervision story.

use std::process::Stdio;
use std::time::Duration;

use lb_run_events::RunEvent;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::profile::AgentProfile;
use crate::wrapper::{AgentWrapper, Decoded};

/// What can go wrong driving the subprocess. Decode failures are **not** here — a bad line is tolerated
/// (the wrapper returns [`Decoded::Ignore`]), matching the forward-compatibility stance for a
/// third-party stream.
#[derive(Debug, thiserror::Error)]
pub enum DriveError {
    /// The agent binary could not be spawned (missing on `PATH`, not executable, …).
    #[error("failed to spawn agent binary {binary:?}: {source}")]
    Spawn {
        binary: String,
        source: std::io::Error,
    },
    /// The subprocess did not finish within the liveness bound and was killed.
    #[error("agent run exceeded {0:?} and was killed")]
    Timeout(Duration),
    /// An I/O error reading the subprocess's stdout.
    #[error("reading agent stdout: {0}")]
    Io(#[from] std::io::Error),
}

/// Drive one run to completion with `wrapper`, collecting every projected [`RunEvent`] **and**, when a
/// `sink` is given, emitting each event **the moment its stdout line decodes** — so a watcher sees the
/// agent work live (tool calls, reasoning, text) instead of a burst at the end. The collected `Vec` is
/// still returned (the caller assembles the final answer from it); the sink is an additive live tap.
///
/// `goal` is the prompt; `workspace` is the cwd the agent runs in. The caller owns env (the API-key
/// var the profile names must be set in this process; the wrapper passes the *name*, never the value).
/// `sink` is an unbounded channel (never blocks the read loop); a closed receiver is ignored (the run
/// keeps going and still returns its collected events).
pub async fn drive(
    wrapper: &dyn AgentWrapper,
    profile: &AgentProfile,
    goal: &str,
    workspace: &str,
    timeout: Duration,
    sink: Option<&tokio::sync::mpsc::UnboundedSender<RunEvent>>,
) -> Result<Vec<RunEvent>, DriveError> {
    let args = wrapper.command_args(profile, goal, workspace);
    let mut child = Command::new(&profile.binary)
        .args(&args)
        .current_dir(workspace)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|source| DriveError::Spawn {
            binary: profile.binary.clone(),
            source,
        })?;

    let stdout = child.stdout.take().expect("stdout was piped");
    let collect = async {
        let mut events = Vec::new();
        let mut turn: u32 = 0;
        // Collect an event AND tap it to the live sink (best-effort: a closed sink never fails the run).
        let mut emit = |event: RunEvent, events: &mut Vec<RunEvent>| {
            if let Some(tx) = sink {
                let _ = tx.send(event.clone());
            }
            events.push(event);
        };
        emit(
            RunEvent::RunStart {
                goal: goal.to_string(),
            },
            &mut events,
        );

        let mut lines = BufReader::new(stdout).lines();
        while let Some(line) = lines.next_line().await? {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match wrapper.decode_line(line, turn) {
                // A model message is a step boundary in the per-step v1 model.
                Decoded::Message(projected) => {
                    emit(RunEvent::StepStart { turn }, &mut events);
                    for ev in projected {
                        emit(ev, &mut events);
                    }
                    turn = turn.saturating_add(1);
                }
                Decoded::Events(projected) => {
                    for ev in projected {
                        emit(ev, &mut events);
                    }
                }
                Decoded::Ignore => {}
            }
        }
        Ok::<_, DriveError>(events)
    };

    match tokio::time::timeout(timeout, collect).await {
        Ok(result) => {
            let _ = child.wait().await;
            result
        }
        Err(_) => {
            let _ = child.kill().await;
            Err(DriveError::Timeout(timeout))
        }
    }
}
