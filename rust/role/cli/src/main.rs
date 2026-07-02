//! `lb` — the operator CLI binary. A THIN shell (FILE-LAYOUT): parse args, run the client library,
//! print the result, map an error to an exit code. All behavior — transport, config, output shaping,
//! the `ws/user/role` header — lives in `lb_cli`, tested in-process against a real gateway.
//!
//! Output convention: the header goes to STDERR (context), the body to STDOUT (the data a pipe
//! consumes) — so `-o json` yields a clean, header-free JSON stream on stdout. An error (including an
//! honest DENY) goes to stderr and exits non-zero — never a fabricated success on stdout.

use std::process::ExitCode;

use lb_cli::cli::Cli;

#[tokio::main]
async fn main() -> ExitCode {
    // `parse_argv` supports the `lb local <cmd>` posture prefix (rewritten to `--local`).
    let cli = Cli::parse_argv();
    match lb_cli::run(cli).await {
        Ok(printed) => {
            // Header → stderr (context, never mixed into piped data); body → stdout.
            eprintln!("{}", printed.header);
            println!("{}", printed.body);
            ExitCode::SUCCESS
        }
        Err(e) => {
            // The honest-deny / clear-error path: the message to stderr, a non-zero code a script can
            // branch on. `DENIED  mcp:<tool>:call` is rendered by `CliError::Display`.
            eprintln!("{e}");
            ExitCode::from(e.exit_code() as u8)
        }
    }
}
