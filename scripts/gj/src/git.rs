//! The actual work: `git add -A` → commit (if dirty) → push. No AI; the message is a template.
//!
//! Mirrors `lazybones-gh`'s `SyncRepo::commit_and_push`: a clean tree is a no-op
//! ([`Pushed::Clean`]), so running this on a timer when nothing changed costs nothing.

use std::process::Command;

use anyhow::{bail, Context, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pushed {
    /// A commit was made and pushed.
    Committed,
    /// Nothing changed; no commit, no push.
    Clean,
}

/// Stage everything, and if the index differs from `HEAD`, commit with the filled
/// `message_tpl` and push to `origin/<branch>` (current branch when `branch` is `None`).
pub fn commit_and_push(repo: &str, branch: Option<&str>, message_tpl: &str) -> Result<Pushed> {
    git(repo, &["add", "-A"])?;

    // `diff --cached --quiet` exits 0 when there is nothing staged ⇒ clean ⇒ no-op.
    if status(repo, &["diff", "--cached", "--quiet"])?.success() {
        return Ok(Pushed::Clean);
    }

    let count = staged_count(repo)?;
    let message = fill_message(message_tpl, count);
    git(repo, &["commit", "-m", &message])?;

    let branch = match branch {
        Some(branch) => branch.to_string(),
        None => current_branch(repo)?,
    };
    git(repo, &["push", "origin", &branch])?;
    Ok(Pushed::Committed)
}

/// The repo's current checked-out branch.
pub fn current_branch(repo: &str) -> Result<String> {
    git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])
}

/// Number of files staged in the index (for the `{n}` placeholder).
fn staged_count(repo: &str) -> Result<usize> {
    let out = git(repo, &["diff", "--cached", "--name-only"])?;
    Ok(out.lines().filter(|line| !line.trim().is_empty()).count())
}

/// Fill `{n}` (file count) and `{t}` (UTC timestamp) in the message template.
fn fill_message(template: &str, count: usize) -> String {
    template
        .replace("{n}", &count.to_string())
        .replace("{t}", &utc_now())
}

/// `date -u +%Y-%m-%dT%H:%M:%SZ` — zero-dep on a Linux/systemd host (where `gj` runs).
fn utc_now() -> String {
    Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_else(|| "now".to_string())
}

/// Run `git -C <repo> <args>` and return trimmed stdout, bailing on a non-zero exit.
fn git(repo: &str, args: &[&str]) -> Result<String> {
    let mut full = vec!["-C", repo];
    full.extend_from_slice(args);
    let out = Command::new("git")
        .args(&full)
        .output()
        .with_context(|| format!("spawn git {}", args.join(" ")))?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let mut stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    while stdout.ends_with(['\n', '\r']) {
        stdout.pop();
    }
    Ok(stdout)
}

/// Run `git -C <repo> <args>` for its exit status only (used for the `--quiet` probe,
/// where a non-zero exit is meaningful, not an error).
fn status(repo: &str, args: &[&str]) -> Result<std::process::ExitStatus> {
    let mut full = vec!["-C", repo];
    full.extend_from_slice(args);
    Command::new("git")
        .args(&full)
        .status()
        .with_context(|| format!("spawn git {}", args.join(" ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real repo + a real bare `file://` remote (no network, no GitHub): prove
    /// commit+push when dirty, and a true no-op when clean.
    #[test]
    fn commit_push_then_clean_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let remote = tmp.path().join("remote.git");
        let work = tmp.path().join("work");

        run(&["git", "init", "-q", "--bare", "-b", "main", remote.to_str().unwrap()]);
        run(&["git", "clone", "-q", remote.to_str().unwrap(), work.to_str().unwrap()]);
        let w = work.to_str().unwrap();
        run(&["git", "-C", w, "config", "user.email", "t@t"]);
        run(&["git", "-C", w, "config", "user.name", "t"]);

        // Dirty → committed and pushed.
        std::fs::write(work.join("a.txt"), "1").unwrap();
        assert_eq!(
            commit_and_push(w, Some("main"), "auto {n}").unwrap(),
            Pushed::Committed
        );
        // The remote actually advanced: the clone's `origin/main` now points at our commit.
        let log = git(w, &["log", "--oneline", "origin/main"]).unwrap();
        assert!(log.contains("auto 1"), "origin/main log was: {log}");

        // Clean → no-op.
        assert_eq!(
            commit_and_push(w, Some("main"), "auto {n}").unwrap(),
            Pushed::Clean
        );

        // Change again → committed.
        std::fs::write(work.join("a.txt"), "2").unwrap();
        assert_eq!(
            commit_and_push(w, Some("main"), "auto {n}").unwrap(),
            Pushed::Committed
        );
    }

    fn run(args: &[&str]) {
        let ok = Command::new(args[0])
            .args(&args[1..])
            .status()
            .unwrap()
            .success();
        assert!(ok, "command failed: {}", args.join(" "));
    }
}
