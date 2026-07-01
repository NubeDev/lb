//! The clap command tree (operator-cli scope). Derive-based, so the parse is declarative and the
//! `--help`/completion surface is free. Global options (`-w`, `-o`, `--url`, `--local`) sit on the top
//! [`Cli`]; the subcommands are the v1 slice: `login`, `whoami`, `call`, `inbox list`, `ext publish`,
//! `devkit sign`, and the `local` posture wrapper. `main.rs` maps this into the client library — the
//! tree carries no logic (FILE-LAYOUT: parsing here, behavior in the command modules).

use clap::{Parser, Subcommand};

/// `lb` — the operator CLI, the terminal twin of the Lazybones shell. A fourth client of the same
/// gateway surface the browser uses; it holds no authority of its own.
#[derive(Debug, Parser)]
#[command(name = "lb", version, about, long_about = None)]
pub struct Cli {
    /// Credential selector: which stored workspace credential to use (NOT a workspace override — the
    /// token's own workspace always wins; an unstored workspace is a loud error). Defaults to the
    /// config's default workspace / `LB_WORKSPACE`.
    #[arg(short = 'w', long = "workspace", global = true, env = "LB_WORKSPACE")]
    pub workspace: Option<String>,

    /// Output format: `table` (default) or `json` (for scripting). `NO_COLOR` is honored.
    #[arg(short = 'o', long = "output", global = true, default_value = "table")]
    pub output: String,

    /// The gateway base URL (remote mode). Overridden by `LB_GATEWAY_URL`; defaults to the config's
    /// stored URL, then `http://127.0.0.1:8080`.
    #[arg(long = "url", global = true, env = "LB_GATEWAY_URL")]
    pub url: Option<String>,

    /// Run in local mode: embed an in-process node and mint a principal scoped by `-w` (offline, no
    /// gateway). The same as prefixing the command with `lb local`.
    #[arg(long = "local", global = true)]
    pub local: bool,

    #[command(subcommand)]
    pub command: Command,
}

/// The v1 command set. `local` is a posture wrapper: `lb local <cmd>` runs `<cmd>` against the
/// in-process node (equivalent to `--local`).
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Log in to a gateway and store the session token (per-workspace, 0600, never logged).
    Login {
        /// The login user (dev-login accepts any). Defaults to `user:ada`.
        #[arg(long)]
        user: Option<String>,
    },
    /// Print who you are: workspace, user, role, and the caps your session carries. Never the token.
    Whoami,
    /// Call any MCP tool directly: the universal escape hatch through `POST /mcp/call`.
    Call {
        /// The qualified tool name, e.g. `hello.echo` or `system.overview`.
        tool: String,
        /// The tool's JSON args (default `{}`).
        args: Option<String>,
    },
    /// Inbox operations (the one typed command in v1).
    #[command(subcommand)]
    Inbox(InboxCmd),
    /// Reminder operations — the reference family of the common resource grammar
    /// (`ls`/`show`/`create`/`update`/`rm` over `reminder.list|get|create|update|delete`).
    #[command(subcommand)]
    Reminder(ReminderCmd),
    /// Extension operations (publish a signed artifact — the `make publish-ext` retirement).
    #[command(subcommand)]
    Ext(ExtCmd),
    /// Developer kit (sign a built extension into a publishable artifact — the `lb-pack` fold).
    #[command(subcommand)]
    Devkit(DevkitCmd),
}

impl Cli {
    /// Parse argv, supporting the `lb local <cmd>` posture prefix as sugar for `--local <cmd>`. A
    /// recursive `local` SUBCOMMAND would make clap's derive help-generation recurse infinitely (a
    /// `Box<Command>` self-reference), so the prefix is rewritten to the global `--local` flag BEFORE
    /// clap parses — one place, no recursion. `lb local login` still errors loudly downstream (login
    /// is remote-only).
    pub fn parse_argv() -> Self {
        let mut args: Vec<String> = std::env::args().collect();
        // args[0] is the binary; a leading `local` (args[1]) becomes `--local`.
        if args.get(1).map(String::as_str) == Some("local") {
            args[1] = "--local".to_string();
        }
        <Self as clap::Parser>::parse_from(args)
    }
}

/// `lb inbox …`.
#[derive(Debug, Subcommand)]
pub enum InboxCmd {
    /// List the inbox for a channel (`inbox.list` → shaped table).
    List {
        /// The channel whose inbox to list.
        channel: String,
    },
}

/// `lb reminder …` — the common resource grammar over the shipped `reminder.*` MCP verbs. Each
/// subcommand is typed sugar over one verb through `POST /mcp/call`; no new verb, cap, or table.
#[derive(Debug, Subcommand)]
pub enum ReminderCmd {
    /// List reminders (`reminder.list`) — ws-scoped, `--status enabled|disabled`, `--limit N`.
    Ls {
        /// Filter by the on/off switch: `enabled` or `disabled`. Omitted → both.
        #[arg(long)]
        status: Option<String>,
        /// Cap the number of rows returned (a bounded head of the sorted set).
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Show one reminder by id (`reminder.get`).
    Show {
        /// The reminder id.
        id: String,
    },
    /// Create a reminder (`reminder.create`). Prints the new id (resource-verbs D4), not the record.
    /// The id is derived from the body unless `--id` is given; the schedule is a 5-field cron string.
    Create {
        /// The channel to post into (the channel-post action — the v1 default action kind).
        #[arg(long)]
        channel: String,
        /// The message body to post when the reminder fires.
        #[arg(long)]
        body: String,
        /// The 5-field cron schedule (e.g. `0 9 * * 1` = Mondays 09:00). Defaults to daily 09:00.
        #[arg(long, default_value = "0 9 * * *")]
        cron: String,
        /// An explicit id. Omitted → derived from the body plus a short unique suffix.
        #[arg(long)]
        id: Option<String>,
        /// Stop after this many firings (min 1). Omitted → recurring forever.
        #[arg(long)]
        max_runs: Option<u32>,
    },
    /// Update a reminder (`reminder.update`) — pause/resume via `--enabled`, reschedule via `--cron`.
    Update {
        /// The reminder id.
        id: String,
        /// Pause (`false`) or resume (`true`). Resuming re-anchors the next fire to the next slot.
        #[arg(long)]
        enabled: Option<bool>,
        /// A new 5-field cron schedule.
        #[arg(long)]
        cron: Option<String>,
        /// A new run cap (min 1).
        #[arg(long)]
        max_runs: Option<u32>,
    },
    /// Delete a reminder (`reminder.delete`) — soft tombstone (it never fires or lists again).
    Rm {
        /// The reminder id.
        id: String,
        /// Purge rather than tombstone. (The shipped verb tombstones; `--hard` is accepted for the
        /// common grammar and passed through — the server currently treats delete as soft.)
        #[arg(long)]
        hard: bool,
    },
}

/// `lb ext …`.
#[derive(Debug, Subcommand)]
pub enum ExtCmd {
    /// Publish a signed artifact JSON, or sign+publish an extension name/dir.
    Publish {
        /// A signed `artifact.json`, or an extension name/dir to sign on the fly.
        target: String,
    },
}

/// `lb devkit …`.
#[derive(Debug, Subcommand)]
pub enum DevkitCmd {
    /// Sign a built extension into the artifact JSON the gateway verifies.
    Sign {
        /// The extension name (under the devkit root) or a path to its dir.
        target: String,
        /// Write the artifact JSON here instead of stdout.
        #[arg(long)]
        out: Option<String>,
    },
}
