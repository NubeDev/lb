//! Dispatch a parsed [`Cli`] to a command, resolving the transport once and routing every verb through
//! it. This is the seam between the clap tree and the command modules — the one place that decides
//! remote vs local (a config/flag choice via [`RunContext::transport`], never an `if cloud` branch).
//! Kept out of `main.rs` so it is unit-testable and `main` stays a thin shell.
//!
//! `login` and `devkit sign` are special: `login` mutates + saves the config (and is remote-only);
//! `devkit sign` is an offline client op needing no transport. Everything else resolves an
//! [`AnyTransport`](crate::transport::AnyTransport) and calls it. A DENY or a down-gateway returns a
//! `CliError` the caller renders.

use crate::cli::{Cli, Command, DevkitCmd, ExtCmd, InboxCmd};
use crate::commands::{call, devkit, ext, inbox, login, whoami, Printed};
use crate::config;
use crate::context::{resolve_gateway_url, RunContext};
use crate::error::{CliError, CliResult};
use crate::header::Header;
use crate::output::Format;
use crate::transport::Transport;

/// Run the parsed CLI end to end: load config, resolve context + format, dispatch. Returns the
/// command's [`Printed`] output; errors (incl. honest denies) propagate for the caller to render.
pub async fn run(cli: Cli) -> CliResult<Printed> {
    let format = Format::parse(&cli.output)?;
    let config = config::load()?;
    let gateway_url = resolve_gateway_url(cli.url.as_deref(), &config);

    // The `lb local <cmd>` prefix was rewritten to the `--local` flag in `Cli::parse_argv`.
    let ctx = RunContext {
        workspace: cli.workspace.clone(),
        gateway_url,
        local: cli.local,
        config,
    };
    dispatch(cli.command, &ctx, format).await
}

/// Route one command under the resolved context.
async fn dispatch(command: Command, ctx: &RunContext, format: Format) -> CliResult<Printed> {
    match command {
        Command::Login { user } => run_login(ctx, user).await,

        Command::Devkit(DevkitCmd::Sign { target, out }) => {
            // Signing needs no transport; still print a header for a consistent surface, using
            // whichever session (local or remote) the run resolved — without requiring a login.
            let header = resolve_header(ctx);
            devkit::sign(&header, &target, out.as_deref())
        }

        Command::Whoami => {
            let t = ctx.transport().await?;
            whoami::render(&t.header(), &t.caps(), format)
        }
        Command::Call { tool, args } => {
            let t = ctx.transport().await?;
            call::run(&t, &tool, args.as_deref(), format).await
        }
        Command::Inbox(InboxCmd::List { channel }) => {
            let t = ctx.transport().await?;
            inbox::list(&t, &channel, format).await
        }
        Command::Ext(ExtCmd::Publish { target }) => run_publish(ctx, &target).await,
    }
}

/// Log in (remote-only) and persist the config (0600). A `--local login` is meaningless — local mints
/// in-process — so this errors loudly rather than silently ignoring the flag.
async fn run_login(ctx: &RunContext, user: Option<String>) -> CliResult<Printed> {
    if ctx.local {
        return Err(CliError::BadInput(
            "`lb login` is remote-only; local mode mints a principal in-process (no login needed)"
                .into(),
        ));
    }
    let workspace = ctx
        .resolve_workspace()
        .ok_or_else(|| CliError::BadInput("login needs a workspace: `lb login -w <ws>`".into()))?;
    let user = user.unwrap_or_else(|| login::DEFAULT_USER.to_string());
    let mut config = ctx.config.clone();
    let printed = login::run(&mut config, &ctx.gateway_url, &user, &workspace).await?;
    config::save(&config)?;
    Ok(printed)
}

/// Publish an extension: build the matching transport (Remote or Local, both implement `ExtPublish`)
/// and call it. Publish is a dedicated route (not `/mcp/call`), so it dispatches on the concrete
/// transport rather than the generic `AnyTransport::call`.
async fn run_publish(ctx: &RunContext, target: &str) -> CliResult<Printed> {
    if ctx.local {
        let t = ctx.local().await?;
        let header = t.header();
        ext::publish(&t, &header, target).await
    } else {
        let t = ctx.remote()?;
        let header = t.header();
        ext::publish(&t, &header, target).await
    }
}

/// Resolve a session header for a NON-transport command (`devkit sign`) — a purely offline op that
/// still prints the context line. It must NOT require a live credential or boot a node just to render
/// a header: if a session is trivially available (a stored remote token) use it, else fall back to a
/// lightweight header from the resolved workspace. This keeps `lb devkit sign` usable before any login
/// (its whole point is producing an artifact to publish LATER).
fn resolve_header(ctx: &RunContext) -> Header {
    let ws = ctx.resolve_workspace().unwrap_or_else(|| "-".to_string());
    if ctx.local {
        // A local sign labels itself local without booting a node (the sign reads no store).
        return Header::new(
            ws,
            crate::context::DEFAULT_LOCAL_USER,
            lb_auth::Role::Member,
            true,
        );
    }
    // Remote: decode the stored token's identity if one exists; else a placeholder user (no login yet).
    if let Ok(remote) = ctx.remote() {
        return remote.header();
    }
    Header::new(ws, "-", lb_auth::Role::Member, false)
}
