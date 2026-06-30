//! clap command-line interface. One responsibility: define the argument schema.
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "claude-switch",
    version,
    about = "Toggle Claude Code between Claude and GLM (z.ai) server configurations",
    long_about = "Manages a set of named server configurations in YAML and writes the \
                  active one into ~/.claude/settings.json. See \
                  https://docs.z.ai/devpack/tool/claude for the GLM coding plan."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Default, Subcommand)]
pub enum Command {
    /// Show the active provider and how it compares to settings.json (default)
    #[default]
    #[command(visible_alias = "st")]
    Status,
    /// List all configured providers
    #[command(visible_alias = "ls")]
    List,
    /// Switch to a provider and apply it to settings.json
    #[command(visible_alias = "sw")]
    Use {
        /// Provider name (e.g. `glm` or `claude`).
        name: String,
        /// Suppress the provider detail dump after switching.
        #[arg(short, long)]
        quiet: bool,
    },
    /// Re-apply the currently active provider to settings.json
    Apply,
    /// Show the full details of one provider
    Show { name: String },
    /// Add or replace a provider in the config
    #[command(visible_alias = "new")]
    Add {
        /// Provider name to create/overwrite.
        name: String,
        /// Base URL written to `ANTHROPIC_BASE_URL`.
        #[arg(short, long)]
        base_url: Option<String>,
        /// Auth token written to `ANTHROPIC_AUTH_TOKEN`.
        #[arg(short, long)]
        token: Option<String>,
        /// Free-form description shown in `list`.
        #[arg(short = 'd', long)]
        description: Option<String>,
        /// Extra `env` entries, `KEY=VALUE` (repeatable).
        #[arg(short = 'e', long = "env", value_name = "KEY=VALUE")]
        extra_env: Vec<String>,
    },
    /// Remove a provider from the config
    #[command(visible_alias = "rm")]
    Remove { name: String },
    /// Open the YAML config in `$EDITOR` (defaults to `vi`)
    Edit,
    /// Print the path of the YAML config file
    Where,
}
