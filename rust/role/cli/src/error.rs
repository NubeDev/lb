//! The CLI error type and its exit-code mapping (operator-cli scope: "denies rendered honestly and
//! exit non-zero, never invented as success").
//!
//! Every failure the operator can hit funnels through [`CliError`] so `main.rs` maps it to ONE exit
//! code and ONE stderr line. The load-bearing variant is [`CliError::Denied`]: a server `403` (remote)
//! or a `ToolError::Denied` (local) is surfaced VERBATIM as `DENIED  mcp:<tool>:call` and exits
//! non-zero — the CLI relays the server's decision, it never fabricates a success (the scope's central
//! honesty invariant + the mandatory capability-deny test).

use std::fmt;

/// A CLI-level failure. The kinds are distinguished so `main` can render each honestly (a deny is
/// NOT a generic error — it is the server's authorization decision, printed as such) and pick an exit
/// code a script can branch on.
#[derive(Debug)]
pub enum CliError {
    /// The server (or the local host) DENIED the tool call — the caller's token/principal lacks
    /// `mcp:<tool>:call`, or the workspace wall refused it. Carries the qualified tool so the render is
    /// `DENIED  mcp:<tool>:call`. Exit code 3. NEVER printed as success.
    Denied { tool: String },
    /// No stored credential for the selected workspace — `-w <ws>` names a workspace the config has no
    /// token for. A LOUD client-side error (the scope's `-w` credential-selector rule), never a silent
    /// ignore. Exit code 4.
    NoCredential { workspace: String },
    /// The remote gateway was unreachable / the request failed at the transport layer (a down gateway,
    /// a DNS error, a refused connection). A clear error, not a hang and not a fake success (the
    /// mandatory offline test for remote mode). Exit code 5.
    Transport(String),
    /// The command's inputs were wrong (bad JSON args, a missing file, an unparsable manifest). Exit
    /// code 2.
    BadInput(String),
    /// Anything else (config IO, serialization) — a generic operator-facing failure. Exit code 1.
    Other(String),
}

impl CliError {
    /// The process exit code for this failure. Distinct codes so a script can tell a DENY (3) from an
    /// unstored-workspace error (4) from a down gateway (5) — the CLI is scriptable (`-o json`), and
    /// the exit code is part of that contract.
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::Other(_) => 1,
            CliError::BadInput(_) => 2,
            CliError::Denied { .. } => 3,
            CliError::NoCredential { .. } => 4,
            CliError::Transport(_) => 5,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // The verbatim deny line the scope specifies: `DENIED  mcp:<tool>:call`. Two spaces, so it
            // reads as a status column. This is the ONLY place a deny is rendered.
            CliError::Denied { tool } => write!(f, "DENIED  mcp:{tool}:call"),
            CliError::NoCredential { workspace } => write!(
                f,
                "no session for workspace {workspace}; run `lb login -w {workspace}`"
            ),
            CliError::Transport(msg) => write!(f, "gateway unreachable: {msg}"),
            CliError::BadInput(msg) => write!(f, "{msg}"),
            CliError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<anyhow::Error> for CliError {
    fn from(e: anyhow::Error) -> Self {
        CliError::Other(e.to_string())
    }
}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        CliError::Other(e.to_string())
    }
}

/// The CLI's result alias — every command returns this, and `main` renders the error once.
pub type CliResult<T> = Result<T, CliError>;
