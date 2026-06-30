//! Argument parsing + subcommand dispatch.

use std::env;
use std::path::PathBuf;

use anyhow::{bail, Result};

use crate::{commands, store};

const HELP: &str = "\
gj - git-jobs: scheduled auto-commit-and-push (no AI)

Usage:
  gj [--file PATH] add <repo> [--every DUR] [--branch B] [--id ID] [--message TPL] [--no-enable]
  gj [--file PATH] ls
  gj [--file PATH] enable <id>
  gj [--file PATH] disable <id>
  gj [--file PATH] rm <id>
  gj [--file PATH] run <id>          # commit+push once now (what the timer calls)
  gj [--file PATH] install <id>      # (re)write + enable the systemd --user timer

Duration (DUR): 30s, 10m, 1h, 2d.   Message template: {n}=file count, {t}=UTC time.
Jobs file: --file PATH | $GJ_FILE | ~/.config/gj/jobs.yaml
Scheduling uses `systemd --user` timers (gj-<id>.timer).";

pub fn run() -> Result<()> {
    let mut args: Vec<String> = env::args().skip(1).collect();

    // Pull the global --file (and --help) out, wherever they appear.
    let mut file = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                println!("{HELP}");
                return Ok(());
            }
            "--file" => {
                if i + 1 >= args.len() {
                    bail!("--file needs a path");
                }
                file = Some(PathBuf::from(args.remove(i + 1)));
                args.remove(i);
            }
            _ => i += 1,
        }
    }

    let store_path = store::path(file)?;

    let Some(command) = args.first().cloned() else {
        return commands::ls(&store_path); // bare `gj` lists
    };
    let rest = args[1..].to_vec();

    match command.as_str() {
        "add" => commands::add(&store_path, rest),
        "ls" | "list" => commands::ls(&store_path),
        "enable" => commands::set_enabled(&store_path, rest, true),
        "disable" => commands::set_enabled(&store_path, rest, false),
        "rm" | "remove" | "delete" => commands::remove(&store_path, rest),
        "run" => commands::run_once(&store_path, rest),
        "install" => commands::install(&store_path, rest),
        other => bail!("unknown command '{other}'\n\n{HELP}"),
    }
}
