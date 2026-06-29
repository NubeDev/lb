//! claude-switch — entry point.
//!
//! Dispatches a subcommand (or defaults to `status`). Each handler is a thin wrapper
//! over the focused modules: `config` (persistence), `settings` (Claude Code JSON),
//! `apply` (the switch), and `output` (styled terminal output).
mod apply;
mod cli;
mod config;
mod output;
mod settings;

use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;
use colored::Colorize;

use cli::{Cli, Command};
use config::{Config, Provider};
use settings as cc;

fn main() -> ExitCode {
    colored::control::set_override(true);
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            output::error(format!("{e:#}"));
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    let command = cli.command.unwrap_or_default();
    match command {
        Command::Status => status(&Config::load_or_init()?)?,
        Command::List => list(&Config::load_or_init()?)?,
        Command::Use { name, quiet } => {
            apply::run(&mut Config::load_or_init()?, &name, quiet)?;
        }
        Command::Apply => apply::apply_active(&Config::load_or_init()?)?,
        Command::Show { name } => show(&Config::load_or_init()?, &name)?,
        Command::Add {
            name,
            base_url,
            token,
            description,
            extra_env,
        } => add(
            &mut Config::load_or_init()?,
            &name,
            base_url,
            token,
            description,
            extra_env,
        )?,
        Command::Remove { name } => remove(&mut Config::load_or_init()?, &name)?,
        Command::Edit => edit()?,
        Command::Where => {
            output::info(Config::path()?.display().to_string());
        }
    }
    Ok(())
}

fn status(cfg: &Config) -> Result<()> {
    output::header("claude-switch — status");
    let active = cfg.active.as_deref().unwrap_or("(none)");
    let settings_path = cc::path()?;
    output::kv("active", active);
    output::kv("config", Config::path()?.display().to_string().as_str());

    let on_disk = cc::read_env()?;
    let live = match (&cfg.active, &on_disk) {
        (Some(name), Some(disk)) => {
            let provider = cfg.get(name);
            let matches = provider.map(|p| env_matches(&p.env, disk)).unwrap_or(false);
            if matches {
                "in sync".to_string()
            } else {
                "out of sync — run `claude-switch use` to reapply".to_string()
            }
        }
        _ => "settings.json has no `env` block".to_string(),
    };
    output::kv("settings", settings_path.display().to_string().as_str());
    output::kv("live", live);

    if let Some(name) = &cfg.active {
        if let Some(p) = cfg.get(name) {
            println!();
            apply::print_provider(name, p);
        }
    }
    Ok(())
}

/// Does the configured provider env match what is currently in settings.json?
fn env_matches(
    cfg_env: &std::collections::BTreeMap<String, String>,
    disk: &serde_json::Map<String, serde_json::Value>,
) -> bool {
    if cfg_env.len() != disk.len() {
        return false;
    }
    cfg_env
        .iter()
        .all(|(k, v)| disk.get(k).and_then(|x| x.as_str()) == Some(v.as_str()))
}

fn list(cfg: &Config) -> Result<()> {
    output::header("configured providers");
    if cfg.providers.is_empty() {
        output::warn("no providers configured");
        return Ok(());
    }
    let active = cfg.active.as_deref();
    for (name, p) in &cfg.providers {
        let marker = if Some(name.as_str()) == active {
            "●"
        } else {
            "○"
        };
        let base = p.base_url.as_deref().unwrap_or("(no base url)");
        let desc = p.description.as_deref().unwrap_or("");
        println!(
            "  {} {:<10} {} {}",
            marker.green(),
            name.bold(),
            base.cyan(),
            desc.dimmed()
        );
    }
    println!();
    output::info(format!("active: {}", active.unwrap_or("(none)")));
    Ok(())
}

fn show(cfg: &Config, name: &str) -> Result<()> {
    let p = cfg
        .get(name)
        .ok_or_else(|| anyhow::anyhow!("unknown provider '{name}'"))?;
    apply::print_provider(name, p);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn add(
    cfg: &mut Config,
    name: &str,
    base_url: Option<String>,
    token: Option<String>,
    description: Option<String>,
    extra_env: Vec<String>,
) -> Result<()> {
    let mut env = std::collections::BTreeMap::new();
    if let Some(t) = token {
        env.insert("ANTHROPIC_AUTH_TOKEN".into(), t);
    }
    if let Some(u) = &base_url {
        env.insert("ANTHROPIC_BASE_URL".into(), u.clone());
    }
    for pair in &extra_env {
        let (k, v) = pair
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("--env expects KEY=VALUE, got '{pair}'"))?;
        env.insert(k.to_string(), v.to_string());
    }

    let provider = Provider {
        description,
        base_url,
        env,
    };
    cfg.providers.insert(name.to_string(), provider);
    cfg.save()?;
    output::success(format!("saved provider '{name}'"));
    output::info(format!("run `claude-switch use {name}` to activate it"));
    Ok(())
}

fn remove(cfg: &mut Config, name: &str) -> Result<()> {
    if cfg.providers.remove(name).is_none() {
        anyhow::bail!("no provider named '{name}' to remove");
    }
    if cfg.active.as_deref() == Some(name) {
        cfg.active = None;
    }
    cfg.save()?;
    output::success(format!("removed provider '{name}'"));
    Ok(())
}

fn edit() -> Result<()> {
    let path = Config::path()?;
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let status = std::process::Command::new(&editor).arg(&path).status()?;
    if !status.success() {
        anyhow::bail!("editor '{editor}' exited with {status}");
    }
    Ok(())
}
