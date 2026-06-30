//! `gj` — git-jobs. Manage scheduled, AI-free "commit everything and push" jobs.
//!
//! A job is a repo + an interval; `gj` stores it in a YAML file and installs a
//! `systemd --user` timer that calls `gj run <id>` on that interval. The commit/push
//! itself is a no-op when the working tree is clean.

mod cli;
mod commands;
mod git;
mod model;
mod store;
mod systemd;

use std::process::ExitCode;

fn main() -> ExitCode {
    match cli::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("gj: {err:#}");
            ExitCode::FAILURE
        }
    }
}
