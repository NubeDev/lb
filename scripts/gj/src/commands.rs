//! The verb handlers: add / ls / enable / disable / rm / run / install.

use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::git::{self, Pushed};
use crate::model::{default_message, every_seconds, Job, Store};
use crate::{store, systemd};

/// `gj add <repo> [--id ID] [--every DUR] [--branch B] [--message TPL] [--no-enable]`
pub fn add(store_path: &Path, args: Vec<String>) -> Result<()> {
    let mut repo = None;
    let (mut id, mut every, mut branch, mut message) = (None, None, None, None);
    let mut no_enable = false;

    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--id" => id = Some(value(&mut it, "--id")?),
            "--every" => every = Some(value(&mut it, "--every")?),
            "--branch" => branch = Some(value(&mut it, "--branch")?),
            "--message" => message = Some(value(&mut it, "--message")?),
            "--no-enable" => no_enable = true,
            flag if flag.starts_with('-') => bail!("unknown flag '{flag}' for add"),
            positional => {
                if repo.replace(positional.to_string()).is_some() {
                    bail!("add takes one repo path");
                }
            }
        }
    }

    let repo = repo.unwrap_or_else(|| ".".to_string());
    let repo_abs = std::fs::canonicalize(&repo)
        .with_context(|| format!("repo path '{repo}' does not exist"))?;
    if !repo_abs.join(".git").exists() {
        eprintln!("gj: warning: {} is not a git repo (no .git)", repo_abs.display());
    }

    let every = every.unwrap_or_else(|| "10m".to_string());
    every_seconds(&every)?; // validate early

    let branch = branch.or_else(|| git::current_branch(&repo_abs.to_string_lossy()).ok());

    let mut store = store::load(store_path)?;
    let id = id.unwrap_or_else(|| gen_id(&store, &repo_abs));
    if store.jobs.iter().any(|job| job.id == id) {
        bail!("job id '{id}' already exists");
    }

    let job = Job {
        id,
        repo: repo_abs.to_string_lossy().into_owned(),
        branch,
        every,
        message: message.unwrap_or_else(default_message),
        enabled: !no_enable,
    };
    store.jobs.push(job.clone());
    store::save(store_path, &store)?;

    println!(
        "added job  {}  every {}  {}",
        job.id,
        job.every,
        job.state_display()
    );

    if job.enabled {
        if let Err(err) = systemd::install(&job, store_path) {
            eprintln!("gj: note: timer not installed ({err:#}).");
            eprintln!("     run `gj install {}` once a user systemd session is available.", job.id);
        } else {
            println!("installed systemd --user timer gj-{}.timer", job.id);
        }
    }
    Ok(())
}

/// `gj ls`
pub fn ls(store_path: &Path) -> Result<()> {
    let store = store::load(store_path)?;
    if store.jobs.is_empty() {
        println!("no jobs. add one:  gj add <repo> --every 10m");
        return Ok(());
    }
    println!(
        "{:<14} {:<8} {:<18} {:<10} REPO",
        "ID", "EVERY", "BRANCH", "STATE"
    );
    for job in &store.jobs {
        println!(
            "{:<14} {:<8} {:<18} {:<10} {}",
            job.id,
            job.every,
            job.branch_display(),
            job.state_display(),
            job.repo
        );
    }
    Ok(())
}

/// `gj enable <id>` / `gj disable <id>`
pub fn set_enabled(store_path: &Path, args: Vec<String>, enabled: bool) -> Result<()> {
    let id = one_id(args, if enabled { "enable" } else { "disable" })?;
    let mut store = store::load(store_path)?;
    let job = find_mut(&mut store, &id)?;
    job.enabled = enabled;
    let snapshot = job.clone();
    store::save(store_path, &store)?;

    if let Err(err) = systemd::set_enabled(&id, enabled, store_path, &snapshot) {
        eprintln!("gj: note: could not {} the timer ({err:#}).", if enabled { "start" } else { "stop" });
    }
    println!("job {id} {}", if enabled { "enabled" } else { "disabled" });
    Ok(())
}

/// `gj rm <id>`
pub fn remove(store_path: &Path, args: Vec<String>) -> Result<()> {
    let id = one_id(args, "rm")?;
    let mut store = store::load(store_path)?;
    let before = store.jobs.len();
    store.jobs.retain(|job| job.id != id);
    if store.jobs.len() == before {
        bail!("no job '{id}'");
    }
    store::save(store_path, &store)?;
    if let Err(err) = systemd::remove(&id) {
        eprintln!("gj: note: could not remove the timer units ({err:#}).");
    }
    println!("removed job {id}");
    Ok(())
}

/// `gj run <id>` — run one commit+push now. This is what the systemd service calls.
pub fn run_once(store_path: &Path, args: Vec<String>) -> Result<()> {
    let id = one_id(args, "run")?;
    let store = store::load(store_path)?;
    let job = store
        .jobs
        .iter()
        .find(|job| job.id == id)
        .with_context(|| format!("no job '{id}'"))?;

    match git::commit_and_push(&job.repo, job.branch.as_deref(), &job.message)? {
        Pushed::Committed => println!("{id}: committed and pushed"),
        Pushed::Clean => println!("{id}: clean, nothing to do"),
    }
    Ok(())
}

/// `gj install <id>` — (re)write + enable the systemd units for an existing job.
pub fn install(store_path: &Path, args: Vec<String>) -> Result<()> {
    let id = one_id(args, "install")?;
    let store = store::load(store_path)?;
    let job = store
        .jobs
        .iter()
        .find(|job| job.id == id)
        .with_context(|| format!("no job '{id}'"))?;
    systemd::install(job, store_path)?;
    println!("installed systemd --user timer gj-{id}.timer");
    Ok(())
}

fn value(it: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    it.next().with_context(|| format!("{flag} needs a value"))
}

fn one_id(args: Vec<String>, verb: &str) -> Result<String> {
    let mut it = args.into_iter().filter(|arg| !arg.starts_with('-'));
    let id = it.next().with_context(|| format!("{verb} needs a job id"))?;
    if it.next().is_some() {
        bail!("{verb} takes one job id");
    }
    Ok(id)
}

fn find_mut<'store>(store: &'store mut Store, id: &str) -> Result<&'store mut Job> {
    store
        .jobs
        .iter_mut()
        .find(|job| job.id == id)
        .with_context(|| format!("no job '{id}'"))
}

/// Derive a stable id from the repo's directory name, deduping on collision.
fn gen_id(store: &Store, repo_abs: &Path) -> String {
    let base = repo_abs
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("job");
    if !store.jobs.iter().any(|job| job.id == base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|candidate| !store.jobs.iter().any(|job| &job.id == candidate))
        .expect("an unused id exists")
}
